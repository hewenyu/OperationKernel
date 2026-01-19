use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;

/// KillShell tool - Terminate background shell processes
pub struct KillShellTool;

#[derive(Debug, Deserialize)]
struct KillShellParams {
    shell_id: String,
}

#[async_trait::async_trait]
impl Tool for KillShellTool {
    fn id(&self) -> &str {
        "kill_shell"
    }

    fn description(&self) -> &str {
        "Terminate a background shell process. \
         Sends SIGKILL to the process and removes it from the registry. \
         Use this to stop long-running commands."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "shell_id": {
                    "type": "string",
                    "description": "Background shell ID to terminate"
                }
            },
            "required": ["shell_id"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: KillShellParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(shell_id = %params.shell_id, "kill_shell start");

        // Check if shell exists
        if !ctx.shell_manager.exists(&params.shell_id).await {
            return Err(ToolError::InvalidParams(format!(
                "Background shell '{}' not found",
                params.shell_id
            )));
        }

        // Get status before killing
        let status_before = ctx
            .shell_manager
            .get_status(&params.shell_id)
            .await
            .ok_or_else(|| {
                ToolError::InvalidParams(format!("Failed to get status for shell '{}'", params.shell_id))
            })?;

        // Kill the shell
        ctx.shell_manager
            .kill(&params.shell_id)
            .await
            .map_err(|e| ToolError::Other(e))?;

        // Remove from registry
        ctx.shell_manager.remove(&params.shell_id).await;

        tracing::info!(shell_id = %params.shell_id, "background shell killed and removed");

        // Format output
        let status_str = match status_before {
            crate::process::background_shell::ShellStatus::Running => "was running",
            crate::process::background_shell::ShellStatus::Completed { exit_code } => {
                &format!("was already completed (exit code: {:?})", exit_code)
            }
            crate::process::background_shell::ShellStatus::Failed { ref error } => {
                &format!("had failed: {}", error)
            }
        };

        let output = format!(
            "Background shell '{}' terminated.\n\
             Status before kill: {}\n\
             Process killed with SIGKILL and removed from registry.",
            params.shell_id, status_str
        );

        // Return result
        Ok(ToolResult::new(
            format!("Killed {}", params.shell_id),
            output,
        )
        .with_metadata("shell_id", json!(params.shell_id))
        .with_metadata("terminated", json!(true)))
    }
}
