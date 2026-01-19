use super::background_shell::{BackgroundShell, ShellStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages all background shell processes
#[derive(Clone)]
pub struct BackgroundShellManager {
    shells: Arc<Mutex<HashMap<String, BackgroundShell>>>,
}

impl BackgroundShellManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            shells: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new background shell and register it
    pub async fn spawn(
        &self,
        id: String,
        command: String,
        working_dir: std::path::PathBuf,
    ) -> anyhow::Result<String> {
        let shell = BackgroundShell::spawn(id.clone(), command, working_dir).await?;

        let shell_id = shell.id().to_string();
        self.shells.lock().await.insert(shell_id.clone(), shell);

        tracing::info!(shell_id = %shell_id, "background shell registered");
        Ok(shell_id)
    }

    /// Check if a shell exists
    pub async fn exists(&self, id: &str) -> bool {
        self.shells.lock().await.contains_key(id)
    }

    /// Get shell status
    pub async fn get_status(&self, id: &str) -> Option<ShellStatus> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(id) {
            Some(shell.status().await)
        } else {
            None
        }
    }

    /// Get stdout lines since offset
    pub async fn get_stdout_since(&self, id: &str, offset: usize) -> Option<Vec<String>> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(id) {
            Some(shell.stdout_since(offset).await)
        } else {
            None
        }
    }

    /// Get stderr lines since offset
    pub async fn get_stderr_since(&self, id: &str, offset: usize) -> Option<Vec<String>> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(id) {
            Some(shell.stderr_since(offset).await)
        } else {
            None
        }
    }

    /// Get all stdout lines
    pub async fn get_stdout(&self, id: &str) -> Option<Vec<String>> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(id) {
            Some(shell.stdout_lines().await)
        } else {
            None
        }
    }

    /// Get all stderr lines
    pub async fn get_stderr(&self, id: &str) -> Option<Vec<String>> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(id) {
            Some(shell.stderr_lines().await)
        } else {
            None
        }
    }

    /// Get line counts for a shell
    pub async fn get_line_counts(&self, id: &str) -> Option<(usize, usize)> {
        let shells = self.shells.lock().await;
        if let Some(shell) = shells.get(id) {
            Some(shell.line_counts().await)
        } else {
            None
        }
    }

    /// Kill a shell by ID
    pub async fn kill(&self, id: &str) -> anyhow::Result<()> {
        let mut shells = self.shells.lock().await;
        if let Some(shell) = shells.get_mut(id) {
            shell.kill().await?;
            tracing::info!(shell_id = %id, "background shell killed");
        }
        Ok(())
    }

    /// Remove a shell from the registry
    pub async fn remove(&self, id: &str) -> Option<BackgroundShell> {
        let removed = self.shells.lock().await.remove(id);
        if removed.is_some() {
            tracing::info!(shell_id = %id, "background shell removed from registry");
        }
        removed
    }

    /// List all shell IDs
    pub async fn list_ids(&self) -> Vec<String> {
        self.shells.lock().await.keys().cloned().collect()
    }

    /// Get shell count
    pub async fn count(&self) -> usize {
        self.shells.lock().await.len()
    }

    /// Clean up finished shells (optional cleanup method)
    pub async fn cleanup_finished(&self) -> usize {
        let mut shells = self.shells.lock().await;
        let mut to_remove = Vec::new();

        for (id, shell) in shells.iter() {
            let status = shell.status().await;
            if matches!(status, ShellStatus::Completed { .. } | ShellStatus::Failed { .. }) {
                to_remove.push(id.clone());
            }
        }

        let count = to_remove.len();
        for id in to_remove {
            shells.remove(&id);
        }

        if count > 0 {
            tracing::info!(cleaned = count, "cleaned up finished shells");
        }
        count
    }

    /// Get summary of all shells
    pub async fn summary(&self) -> Vec<ShellSummary> {
        let shells = self.shells.lock().await;
        let mut summaries = Vec::new();

        for shell in shells.values() {
            let status = shell.status().await;
            let (stdout_lines, stderr_lines) = shell.line_counts().await;

            summaries.push(ShellSummary {
                id: shell.id().to_string(),
                status,
                stdout_lines,
                stderr_lines,
                uptime_secs: shell.uptime_secs(),
            });
        }

        summaries
    }
}

impl Default for BackgroundShellManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary information about a background shell
#[derive(Debug, Clone)]
pub struct ShellSummary {
    pub id: String,
    pub status: ShellStatus,
    pub stdout_lines: usize,
    pub stderr_lines: usize,
    pub uptime_secs: u64,
}
