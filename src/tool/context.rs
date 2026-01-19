use std::path::PathBuf;

use super::base::ToolContext;

impl ToolContext {
    /// Create a new tool context
    pub fn new(
        session_id: impl Into<String>,
        message_id: impl Into<String>,
        agent: impl Into<String>,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            message_id: message_id.into(),
            agent: agent.into(),
            working_dir,
        }
    }

    /// Create a default context with current working directory
    pub fn default_with_cwd() -> std::io::Result<Self> {
        Ok(Self {
            session_id: "default".to_string(),
            message_id: "default".to_string(),
            agent: "default".to_string(),
            working_dir: std::env::current_dir()?,
        })
    }
}
