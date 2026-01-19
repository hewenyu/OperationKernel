use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;

/// Bash tool - executes shell commands and returns output
pub struct BashTool;

#[derive(Debug, Deserialize)]
struct BashParams {
    command: String,
    #[serde(default = "default_timeout")]
    timeout: u64, // milliseconds
    #[serde(default)]
    description: String,
}

fn default_timeout() -> u64 {
    120_000 // 2 minutes default
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn id(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute shell commands and capture stdout/stderr. \
         Supports timeout control (default 2 minutes). \
         Returns exit code and combined output."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000 = 2 minutes)",
                    "default": 120000
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of what this command does",
                    "default": ""
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: BashParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        // 1. Create command process
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&params.command)
            .current_dir(&ctx.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ToolError::Other(e.into()))?;

        // 2. Set timeout duration
        let timeout = Duration::from_millis(params.timeout);

        // 3. Collect output with timeout
        let result = tokio::time::timeout(timeout, async {
            // Get stdout and stderr pipes
            let mut stdout = child.stdout.take().expect("Failed to capture stdout");
            let mut stderr = child.stderr.take().expect("Failed to capture stderr");

            // Read both streams concurrently
            let (stdout_result, stderr_result) = tokio::join!(
                read_to_string(&mut stdout),
                read_to_string(&mut stderr)
            );

            // Wait for process to exit
            let exit_status = child.wait().await?;

            Ok::<_, anyhow::Error>((
                stdout_result?,
                stderr_result?,
                exit_status.code(),
            ))
        })
        .await;

        // 4. Handle timeout
        let (stdout, stderr, exit_code) = match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(ToolError::Other(e));
            }
            Err(_) => {
                // Timeout - try to kill the process
                let _ = child.kill().await;
                return Err(ToolError::Timeout(params.timeout));
            }
        };

        // 5. Format output
        let mut final_output = String::new();

        if !stdout.is_empty() {
            final_output.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !final_output.is_empty() {
                final_output.push_str("\n\n--- STDERR ---\n");
            }
            final_output.push_str(&stderr);
        }

        if final_output.is_empty() {
            final_output.push_str("(No output)");
        }

        // 6. Determine if command failed
        let title = if !params.description.is_empty() {
            params.description.clone()
        } else {
            format!("$ {}", params.command)
        };

        // 7. Return result
        Ok(ToolResult::new(title, final_output)
            .with_metadata("exit_code", json!(exit_code))
            .with_metadata("command", json!(params.command))
            .with_metadata("success", json!(exit_code == Some(0))))
    }
}

/// Helper function to read a stream to string
async fn read_to_string<R: AsyncReadExt + Unpin>(reader: &mut R) -> anyhow::Result<String> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await?;
    Ok(String::from_utf8_lossy(&buffer).to_string())
}
