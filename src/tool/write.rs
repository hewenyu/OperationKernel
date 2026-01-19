use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use similar::TextDiff;
use std::path::PathBuf;

/// Write tool - writes content to a file (creates or overwrites)
pub struct WriteTool;

#[derive(Debug, Deserialize)]
struct WriteParams {
    file_path: PathBuf,
    content: String,
}

impl WriteTool {
    /// Generate a unified diff between old and new content
    fn generate_diff(filepath: &PathBuf, old: &str, new: &str) -> String {
        let diff = TextDiff::from_lines(old, new);
        let mut output = String::new();

        output.push_str(&format!("--- {}\n", filepath.display()));
        output.push_str(&format!("+++ {}\n", filepath.display()));

        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            output.push_str(&format!("{}{}", sign, change));
        }

        output
    }
}

#[async_trait::async_trait]
impl Tool for WriteTool {
    fn id(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, \
         or overwrites it if it does. Shows a diff of changes for existing files."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to write (absolute or relative)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: WriteParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            working_dir = %ctx.working_dir.display(),
            file_path = %params.file_path.display(),
            bytes = params.content.len(),
            "tool write start"
        );

        // 1. Resolve file path (relative to working directory)
        let filepath = if params.file_path.is_absolute() {
            params.file_path
        } else {
            ctx.working_dir.join(&params.file_path)
        };

        // 2. Read old content if file exists
        let old_content = if filepath.exists() {
            tokio::fs::read_to_string(&filepath).await.ok()
        } else {
            None
        };

        // 3. Generate diff or creation message
        let diff = if let Some(old) = &old_content {
            Self::generate_diff(&filepath, old, &params.content)
        } else {
            format!("Creating new file: {}\n", filepath.display())
        };

        // 4. Create parent directories if needed
        if let Some(parent) = filepath.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| ToolError::Other(e.into()))?;
            }
        }

        // 5. Write the file
        tokio::fs::write(&filepath, &params.content)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        // 6. Build output message
        let mut output = format!("Successfully wrote to: {}\n\n", filepath.display());
        output.push_str(&diff);

        tracing::debug!(
            resolved_path = %filepath.display(),
            existed = old_content.is_some(),
            bytes_written = params.content.len(),
            "tool write done"
        );

        // 7. Return result
        Ok(ToolResult::new(
            filepath.to_string_lossy(),
            output,
        )
        .with_metadata("filepath", json!(filepath.to_string_lossy()))
        .with_metadata("existed", json!(old_content.is_some()))
        .with_metadata("bytes_written", json!(params.content.len())))
    }
}
