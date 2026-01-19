use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Subagent session manager - handles persistence and resume functionality
pub struct SubagentManager {
    storage_dir: PathBuf,
}

/// Persisted subagent session data
#[derive(Debug, Serialize, Deserialize)]
pub struct SubagentSession {
    pub agent_id: String,
    pub subagent_type: String,
    pub parent_session_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub transcript: Vec<serde_json::Value>, // Storing as JSON for flexibility
}

impl SubagentManager {
    /// Create a new subagent manager with default storage location
    pub fn new() -> Self {
        let storage_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ok")
            .join("subagents");

        Self { storage_dir }
    }

    /// Create a subagent manager with custom storage path (for testing)
    pub fn with_storage_path(storage_dir: PathBuf) -> Self {
        Self { storage_dir }
    }

    /// Create a new subagent session and return its unique ID
    pub fn create_session(&self, subagent_type: &str, parent_session_id: &str) -> Result<String> {
        // Generate unique agent ID
        let agent_id = Uuid::new_v4().to_string();

        // Create session data
        let session = SubagentSession {
            agent_id: agent_id.clone(),
            subagent_type: subagent_type.to_string(),
            parent_session_id: parent_session_id.to_string(),
            created_at: chrono::Utc::now(),
            transcript: Vec::new(),
        };

        // Save initial empty session
        self.save_session(&session)?;

        tracing::info!(
            agent_id = %agent_id,
            subagent_type = %subagent_type,
            "created new subagent session"
        );

        Ok(agent_id)
    }

    /// Save a subagent session to disk
    pub fn save_session(&self, session: &SubagentSession) -> Result<()> {
        // Ensure directory exists
        std::fs::create_dir_all(&self.storage_dir)?;

        let filepath = self.storage_dir.join(format!("{}.json", session.agent_id));

        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&filepath, json)?;

        tracing::debug!(
            agent_id = %session.agent_id,
            path = %filepath.display(),
            "saved subagent session"
        );

        Ok(())
    }

    /// Load a subagent session from disk
    pub fn load_session(&self, agent_id: &str) -> Result<SubagentSession> {
        let filepath = self.storage_dir.join(format!("{}.json", agent_id));

        if !filepath.exists() {
            return Err(anyhow!("Subagent session not found: {}", agent_id));
        }

        let content = std::fs::read_to_string(&filepath)?;
        let session: SubagentSession = serde_json::from_str(&content)?;

        tracing::debug!(
            agent_id = %agent_id,
            transcript_len = session.transcript.len(),
            "loaded subagent session"
        );

        Ok(session)
    }

    /// Update session transcript
    pub fn update_transcript(
        &self,
        agent_id: &str,
        transcript: Vec<serde_json::Value>,
    ) -> Result<()> {
        let mut session = self.load_session(agent_id)?;
        session.transcript = transcript;
        self.save_session(&session)?;

        tracing::debug!(
            agent_id = %agent_id,
            transcript_len = session.transcript.len(),
            "updated subagent transcript"
        );

        Ok(())
    }

    /// Check if a session exists
    pub fn session_exists(&self, agent_id: &str) -> bool {
        let filepath = self.storage_dir.join(format!("{}.json", agent_id));
        filepath.exists()
    }

    /// Delete a session
    pub fn delete_session(&self, agent_id: &str) -> Result<()> {
        let filepath = self.storage_dir.join(format!("{}.json", agent_id));

        if filepath.exists() {
            std::fs::remove_file(&filepath)?;
            tracing::debug!(agent_id = %agent_id, "deleted subagent session");
        }

        Ok(())
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        if !self.storage_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        for entry in std::fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(stem.to_string());
                }
            }
        }

        Ok(sessions)
    }

    /// Clean up old sessions (older than 30 days)
    pub fn cleanup_old_sessions(&self, days: u64) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let mut cleaned = 0;

        for session_id in self.list_sessions()? {
            if let Ok(session) = self.load_session(&session_id) {
                if session.created_at < cutoff {
                    self.delete_session(&session_id)?;
                    cleaned += 1;
                }
            }
        }

        if cleaned > 0 {
            tracing::info!(cleaned = cleaned, days = days, "cleaned up old subagent sessions");
        }

        Ok(cleaned)
    }
}

impl Default for SubagentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_session() {
        let temp_dir = tempdir().unwrap();
        let manager = SubagentManager::with_storage_path(temp_dir.path().to_path_buf());

        let agent_id = manager.create_session("Explore", "parent-123").unwrap();

        assert!(!agent_id.is_empty());
        assert!(manager.session_exists(&agent_id));
    }

    #[test]
    fn test_load_session() {
        let temp_dir = tempdir().unwrap();
        let manager = SubagentManager::with_storage_path(temp_dir.path().to_path_buf());

        let agent_id = manager.create_session("Plan", "parent-456").unwrap();
        let session = manager.load_session(&agent_id).unwrap();

        assert_eq!(session.agent_id, agent_id);
        assert_eq!(session.subagent_type, "Plan");
        assert_eq!(session.parent_session_id, "parent-456");
        assert_eq!(session.transcript.len(), 0);
    }

    #[test]
    fn test_update_transcript() {
        let temp_dir = tempdir().unwrap();
        let manager = SubagentManager::with_storage_path(temp_dir.path().to_path_buf());

        let agent_id = manager.create_session("Bash", "parent-789").unwrap();

        let transcript = vec![
            serde_json::json!({"role": "user", "content": "test"}),
            serde_json::json!({"role": "assistant", "content": "response"}),
        ];

        manager.update_transcript(&agent_id, transcript).unwrap();

        let session = manager.load_session(&agent_id).unwrap();
        assert_eq!(session.transcript.len(), 2);
    }

    #[test]
    fn test_delete_session() {
        let temp_dir = tempdir().unwrap();
        let manager = SubagentManager::with_storage_path(temp_dir.path().to_path_buf());

        let agent_id = manager.create_session("Explore", "parent-111").unwrap();
        assert!(manager.session_exists(&agent_id));

        manager.delete_session(&agent_id).unwrap();
        assert!(!manager.session_exists(&agent_id));
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = tempdir().unwrap();
        let manager = SubagentManager::with_storage_path(temp_dir.path().to_path_buf());

        let id1 = manager.create_session("Explore", "parent-1").unwrap();
        let id2 = manager.create_session("Plan", "parent-2").unwrap();

        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&id1));
        assert!(sessions.contains(&id2));
    }

    #[test]
    fn test_nonexistent_session() {
        let temp_dir = tempdir().unwrap();
        let manager = SubagentManager::with_storage_path(temp_dir.path().to_path_buf());

        let result = manager.load_session("nonexistent-id");
        assert!(result.is_err());
    }
}
