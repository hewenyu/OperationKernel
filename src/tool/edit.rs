use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use similar::TextDiff;
use std::path::PathBuf;

/// Edit tool - performs precise string replacements in files
pub struct EditTool;

#[derive(Debug, Deserialize)]
struct EditParams {
    file_path: PathBuf,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

impl EditTool {
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

    /// Find all occurrences of a substring and return their positions
    fn find_occurrences(content: &str, pattern: &str) -> Vec<usize> {
        content
            .match_indices(pattern)
            .map(|(pos, _)| pos)
            .collect()
    }

    /// Calculate line and column for a byte position
    fn position_to_line_col(content: &str, pos: usize) -> (usize, usize) {
        let before = &content[..pos];
        let line = before.lines().count();
        let col = before.lines().last().map(|l| l.len()).unwrap_or(0);
        (line, col)
    }
}

#[async_trait::async_trait]
impl Tool for EditTool {
    fn id(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing exact string matches. Performs precise string replacement \
         with uniqueness validation to prevent accidental modifications. Use replace_all=true \
         to replace all occurrences, or provide enough context to make the match unique."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to edit (absolute or relative)"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to replace (must match exactly, including whitespace)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                },
                "replace_all": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, replace all occurrences. If false (default), will error if multiple matches found."
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: EditParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            working_dir = %ctx.working_dir.display(),
            file_path = %params.file_path.display(),
            old_len = params.old_string.len(),
            new_len = params.new_string.len(),
            replace_all = params.replace_all,
            "tool edit start"
        );

        // 1. Validate that old_string and new_string are different
        if params.old_string == params.new_string {
            return Err(ToolError::OldNewIdentical);
        }

        // 2. Resolve file path (relative to working directory)
        let filepath = if params.file_path.is_absolute() {
            params.file_path
        } else {
            ctx.working_dir.join(&params.file_path)
        };

        // 3. Check file exists
        if !filepath.exists() {
            return Err(ToolError::FileNotFound(filepath));
        }

        // 4. Read file content
        let content = tokio::fs::read_to_string(&filepath)
            .await
            .map_err(|e| {
                // Check if it's a binary file error
                if e.to_string().contains("invalid utf-8") {
                    ToolError::BinaryFile(filepath.clone())
                } else {
                    ToolError::Other(e.into())
                }
            })?;

        // 5. Find all occurrences of old_string
        let positions = Self::find_occurrences(&content, &params.old_string);

        if positions.is_empty() {
            return Err(ToolError::OldStringNotFound(params.old_string.clone()));
        }

        // 6. Validate uniqueness if replace_all is false
        if !params.replace_all && positions.len() > 1 {
            return Err(ToolError::MultipleMatches {
                count: positions.len(),
                positions: positions.clone(),
            });
        }

        // 7. Perform replacement
        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            // Replace only the first occurrence
            content.replacen(&params.old_string, &params.new_string, 1)
        };

        // 8. Generate diff
        let diff = Self::generate_diff(&filepath, &content, &new_content);

        // 9. Write the file
        tokio::fs::write(&filepath, &new_content)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        // 10. Build output message
        let replacement_count = positions.len();
        let mut output = format!(
            "Successfully edited: {}\n",
            filepath.display()
        );
        output.push_str(&format!(
            "Replacements made: {} occurrence(s)\n\n",
            if params.replace_all { replacement_count } else { 1 }
        ));
        output.push_str(&diff);

        tracing::debug!(
            resolved_path = %filepath.display(),
            replacements = if params.replace_all { replacement_count } else { 1 },
            old_size = content.len(),
            new_size = new_content.len(),
            "tool edit done"
        );

        // 11. Return result
        Ok(ToolResult::new(filepath.to_string_lossy(), output)
            .with_metadata("filepath", json!(filepath.to_string_lossy()))
            .with_metadata(
                "replacements",
                json!(if params.replace_all { replacement_count } else { 1 })
            )
            .with_metadata("old_length", json!(params.old_string.len()))
            .with_metadata("new_length", json!(params.new_string.len()))
            .with_metadata("replace_all", json!(params.replace_all)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    fn create_test_context() -> ToolContext {
        ToolContext {
            session_id: "test-session".to_string(),
            message_id: "test-message".to_string(),
            agent: "test-agent".to_string(),
            working_dir: PathBuf::from("/tmp"),
        }
    }

    #[tokio::test]
    async fn test_simple_replace() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello World").unwrap();
        writeln!(file, "Goodbye World").unwrap();
        file.flush().unwrap();

        let tool = EditTool;
        let params = json!({
            "file_path": file.path(),
            "old_string": "Hello World",
            "new_string": "Hi Earth",
            "replace_all": false
        });

        let ctx = create_test_context();
        let result = tool.execute(params, &ctx).await.unwrap();

        assert!(result.output.contains("Hi Earth"));
        assert_eq!(result.metadata.get("replacements").unwrap(), &json!(1));

        // Verify file content
        let content = std::fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("Hi Earth"));
        assert!(!content.contains("Hello World"));
    }

    #[tokio::test]
    async fn test_replace_all() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo bar foo").unwrap();
        writeln!(file, "foo baz").unwrap();
        file.flush().unwrap();

        let tool = EditTool;
        let params = json!({
            "file_path": file.path(),
            "old_string": "foo",
            "new_string": "qux",
            "replace_all": true
        });

        let ctx = create_test_context();
        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.metadata.get("replacements").unwrap(), &json!(3));

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(content.matches("qux").count(), 3);
        assert!(!content.contains("foo"));
    }

    #[tokio::test]
    async fn test_multiple_matches_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test test test").unwrap();
        file.flush().unwrap();

        let tool = EditTool;
        let params = json!({
            "file_path": file.path(),
            "old_string": "test",
            "new_string": "pass",
            "replace_all": false
        });

        let ctx = create_test_context();
        let result = tool.execute(params, &ctx).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::MultipleMatches { count, .. } => {
                assert_eq!(count, 3);
            }
            _ => panic!("Expected MultipleMatches error"),
        }
    }

    #[tokio::test]
    async fn test_string_not_found() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello World").unwrap();
        file.flush().unwrap();

        let tool = EditTool;
        let params = json!({
            "file_path": file.path(),
            "old_string": "Nonexistent",
            "new_string": "Something",
        });

        let ctx = create_test_context();
        let result = tool.execute(params, &ctx).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::OldStringNotFound(_)));
    }

    #[tokio::test]
    async fn test_identical_strings_error() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello").unwrap();
        file.flush().unwrap();

        let tool = EditTool;
        let params = json!({
            "file_path": file.path(),
            "old_string": "Hello",
            "new_string": "Hello",
        });

        let ctx = create_test_context();
        let result = tool.execute(params, &ctx).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::OldNewIdentical));
    }

    #[tokio::test]
    async fn test_preserve_indentation() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "fn main() {{").unwrap();
        writeln!(file, "    println!(\"old\");").unwrap();
        writeln!(file, "}}").unwrap();
        file.flush().unwrap();

        let tool = EditTool;
        let params = json!({
            "file_path": file.path(),
            "old_string": "    println!(\"old\");",
            "new_string": "    println!(\"new\");",
        });

        let ctx = create_test_context();
        let _result = tool.execute(params, &ctx).await.unwrap();

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("    println!(\"new\");"));
    }
}
