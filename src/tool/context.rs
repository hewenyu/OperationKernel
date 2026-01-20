use std::path::PathBuf;
use std::sync::Arc;

use super::base::ToolContext;
use crate::process::BackgroundShellManager;

impl ToolContext {
    /// Create a new tool context
    pub fn new(
        session_id: impl Into<String>,
        message_id: impl Into<String>,
        agent: impl Into<String>,
        working_dir: PathBuf,
        shell_manager: Arc<BackgroundShellManager>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            message_id: message_id.into(),
            agent: agent.into(),
            working_dir,
            shell_manager,
        }
    }

    /// Create a default context with current working directory
    pub fn default_with_cwd() -> std::io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let working_dir = cwd.canonicalize().unwrap_or(cwd);

        Ok(Self {
            session_id: "default".to_string(),
            message_id: "default".to_string(),
            agent: "default".to_string(),
            working_dir,
            shell_manager: Arc::new(BackgroundShellManager::new()),
        })
    }
}
