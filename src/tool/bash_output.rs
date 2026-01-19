use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;

/// BashOutput tool - Monitor output from background shell processes
pub struct BashOutputTool;

#[derive(Debug, Deserialize)]
struct BashOutputParams {
    shell_id: String,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    filter: Option<String>,
}

#[async_trait::async_trait]
impl Tool for BashOutputTool {
    fn id(&self) -> &str {
        "bash_output"
    }

    fn description(&self) -> &str {
        "Monitor output from a background shell process. \
         Returns new stdout/stderr lines since last check. \
         Supports optional regex filtering."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "shell_id": {
                    "type": "string",
                    "description": "Background shell ID (returned by bash tool with run_in_background=true)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line offset to start reading from (default: 0 = all lines)",
                    "default": 0
                },
                "filter": {
                    "type": "string",
                    "description": "Optional regex pattern to filter output lines"
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
        let params: BashOutputParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            shell_id = %params.shell_id,
            offset = params.offset,
            filter = ?params.filter,
            "bash_output start"
        );

        // Check if shell exists
        if !ctx.shell_manager.exists(&params.shell_id).await {
            return Err(ToolError::InvalidParams(format!(
                "Background shell '{}' not found",
                params.shell_id
            )));
        }

        // Get shell status
        let status = ctx
            .shell_manager
            .get_status(&params.shell_id)
            .await
            .ok_or_else(|| {
                ToolError::InvalidParams(format!("Failed to get status for shell '{}'", params.shell_id))
            })?;

        // Get stdout and stderr since offset
        let mut stdout_lines = ctx
            .shell_manager
            .get_stdout_since(&params.shell_id, params.offset)
            .await
            .unwrap_or_default();
        let mut stderr_lines = ctx
            .shell_manager
            .get_stderr_since(&params.shell_id, params.offset)
            .await
            .unwrap_or_default();

        // Apply filter if specified
        if let Some(ref filter_pattern) = params.filter {
            let regex = regex::Regex::new(filter_pattern)
                .map_err(|e| ToolError::InvalidParams(format!("Invalid regex: {}", e)))?;

            stdout_lines.retain(|line| regex.is_match(line));
            stderr_lines.retain(|line| regex.is_match(line));
        }

        // Get total line counts
        let (total_stdout, total_stderr) = ctx
            .shell_manager
            .get_line_counts(&params.shell_id)
            .await
            .unwrap_or((0, 0));

        // Format output
        let mut output = String::new();

        // Status line
        let status_str = match status {
            crate::process::background_shell::ShellStatus::Running => "Running",
            crate::process::background_shell::ShellStatus::Completed { exit_code } => {
                &format!("Completed (exit code: {:?})", exit_code)
            }
            crate::process::background_shell::ShellStatus::Failed { ref error } => {
                &format!("Failed: {}", error)
            }
        };
        output.push_str(&format!("Status: {}\n", status_str));
        output.push_str(&format!(
            "Total lines: {} stdout, {} stderr\n\n",
            total_stdout, total_stderr
        ));

        // New output since offset
        if !stdout_lines.is_empty() {
            output.push_str("=== STDOUT (new) ===\n");
            for line in &stdout_lines {
                output.push_str(line);
                output.push('\n');
            }
            output.push('\n');
        }

        if !stderr_lines.is_empty() {
            output.push_str("=== STDERR (new) ===\n");
            for line in &stderr_lines {
                output.push_str(line);
                output.push('\n');
            }
            output.push('\n');
        }

        if stdout_lines.is_empty() && stderr_lines.is_empty() {
            output.push_str("(No new output since offset)\n");
        }

        let new_offset = params.offset + stdout_lines.len() + stderr_lines.len();

        tracing::debug!(
            shell_id = %params.shell_id,
            stdout_lines = stdout_lines.len(),
            stderr_lines = stderr_lines.len(),
            new_offset,
            "bash_output done"
        );

        // Return result
        Ok(ToolResult::new(
            format!("Output from {}", params.shell_id),
            output,
        )
        .with_metadata("shell_id", json!(params.shell_id))
        .with_metadata("status", json!(status_str))
        .with_metadata("new_stdout_lines", json!(stdout_lines.len()))
        .with_metadata("new_stderr_lines", json!(stderr_lines.len()))
        .with_metadata("new_offset", json!(new_offset))
        .with_metadata("total_stdout_lines", json!(total_stdout))
        .with_metadata("total_stderr_lines", json!(total_stderr)))
    }
}
