use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

/// Read tool - reads file contents with line numbers and smart truncation
pub struct ReadTool {
    max_line_length: usize,
    max_bytes: usize,
}

impl ReadTool {
    pub fn new() -> Self {
        Self {
            max_line_length: 2000,
            max_bytes: 50 * 1024, // 50KB
        }
    }

    /// Check if a file is likely binary by examining extension and content
    async fn is_binary_file(path: &PathBuf) -> anyhow::Result<bool> {
        // 1. Check file extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_str().unwrap_or("");
            const BINARY_EXTS: &[&str] = &[
                "zip", "tar", "gz", "exe", "dll", "so", "jar",
                "wasm", "pyc", "bin", "dat", "db", "sqlite",
                "png", "jpg", "jpeg", "gif", "bmp", "ico",
                "mp3", "mp4", "avi", "mov", "pdf",
            ];
            if BINARY_EXTS.contains(&ext_str) {
                return Ok(true);
            }
        }

        // 2. Check file content (first 4KB)
        let mut file = match tokio::fs::File::open(path).await {
            Ok(f) => f,
            Err(_) => return Ok(false), // Can't open = not binary
        };

        let mut buffer = [0u8; 4096];
        let n = match file.read(&mut buffer).await {
            Ok(n) => n,
            Err(_) => return Ok(false),
        };

        if n == 0 {
            return Ok(false); // Empty file is not binary
        }

        // Check for null bytes (strong indicator of binary)
        if buffer[..n].contains(&0) {
            return Ok(true);
        }

        // Count non-printable characters
        let non_printable = buffer[..n]
            .iter()
            .filter(|&&b| b < 9 || (b > 13 && b < 32))
            .count();

        // If more than 30% non-printable, it's likely binary
        Ok(non_printable as f64 / n as f64 > 0.3)
    }
}

#[derive(Debug, Deserialize)]
struct ReadParams {
    file_path: PathBuf,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    2000
}

#[async_trait::async_trait]
impl Tool for ReadTool {
    fn id(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read file contents with line numbers and smart truncation. \
         Supports offset/limit for large files. Detects binary files."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to read (absolute or relative)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-based, default: 0)",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default: 2000)",
                    "default": 2000
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: ReadParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        // 1. Resolve file path (relative to working directory)
        let filepath = if params.file_path.is_absolute() {
            params.file_path
        } else {
            ctx.working_dir.join(&params.file_path)
        };

        // 2. Check file exists
        if !filepath.exists() {
            return Err(ToolError::FileNotFound(filepath));
        }

        // 3. Check if binary
        if Self::is_binary_file(&filepath).await? {
            return Err(ToolError::BinaryFile(filepath));
        }

        // 4. Read file content
        let content = tokio::fs::read_to_string(&filepath)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // 5. Apply offset and limit
        let offset = params.offset;
        let limit = params.limit;
        let end = (offset + limit).min(total_lines);

        // 6. Format lines with line numbers and truncation
        let mut output_lines = Vec::new();
        let mut bytes_count = 0;
        let mut truncated_by_bytes = false;

        for (idx, line) in lines[offset..end].iter().enumerate() {
            let line_num = offset + idx + 1; // 1-based line numbers

            // Truncate overly long lines
            let truncated_line = if line.len() > self.max_line_length {
                format!("{}... (line truncated)", &line[..self.max_line_length])
            } else {
                line.to_string()
            };

            let formatted = format!("{:>5}\u{2192}{}", line_num, truncated_line);
            let line_bytes = formatted.as_bytes().len() + 1; // +1 for newline

            // Check if we've exceeded max bytes
            if bytes_count + line_bytes > self.max_bytes {
                truncated_by_bytes = true;
                break;
            }

            output_lines.push(formatted);
            bytes_count += line_bytes;
        }

        // 7. Build final output
        let mut final_output = String::new();
        final_output.push_str(&output_lines.join("\n"));
        final_output.push_str("\n\n");

        // Add informative footer
        let last_line = offset + output_lines.len();
        if truncated_by_bytes {
            final_output.push_str(&format!(
                "(Output truncated at {} bytes. Use offset={} to read beyond line {})",
                self.max_bytes, last_line, last_line
            ));
        } else if last_line < total_lines {
            final_output.push_str(&format!(
                "(File has more lines. Use offset={} to read beyond line {})",
                last_line, last_line
            ));
        } else {
            final_output.push_str(&format!("(End of file - {} lines total)", total_lines));
        }

        // 8. Return result
        Ok(ToolResult::new(
            filepath.to_string_lossy(),
            final_output,
        )
        .with_metadata("total_lines", json!(total_lines))
        .with_metadata("lines_read", json!(output_lines.len()))
        .with_metadata("truncated", json!(truncated_by_bytes || last_line < total_lines)))
    }
}
