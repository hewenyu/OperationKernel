use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

/// Glob tool - searches for files matching glob patterns
pub struct GlobTool {
    max_results: usize,
    timeout: Duration,
}

impl GlobTool {
    pub fn new() -> Self {
        Self {
            max_results: 200,
            timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GlobParams {
    pattern: String,
    #[serde(default = "default_path")]
    path: PathBuf,
    #[serde(default = "default_max_results")]
    max_results: usize,
    #[serde(default)]
    show_hidden: bool,
}

fn default_path() -> PathBuf {
    PathBuf::from(".")
}

fn default_max_results() -> usize {
    200
}

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn id(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Search for files matching glob patterns (e.g., '**/*.rs', 'src/**/*.toml'). \
         Supports recursive patterns and respects .gitignore files. \
         Returns sorted list of matching file paths."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., '**/*.rs', 'src/**/*.toml')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)",
                    "default": "."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of files to return (default: 200)",
                    "default": 200
                },
                "show_hidden": {
                    "type": "boolean",
                    "description": "Include hidden files in results (default: false)",
                    "default": false
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: GlobParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            working_dir = %ctx.working_dir.display(),
            pattern = %params.pattern,
            path = %params.path.display(),
            max_results = params.max_results,
            show_hidden = params.show_hidden,
            "tool glob start"
        );

        // 1. Validate glob pattern
        let glob_matcher = globset::Glob::new(&params.pattern)
            .map_err(|e| ToolError::InvalidParams(format!("Invalid glob pattern: {}", e)))?
            .compile_matcher();

        // 2. Resolve search path
        let search_path = if params.path.is_absolute() {
            params.path.clone()
        } else {
            ctx.working_dir.join(&params.path)
        };

        if !search_path.exists() {
            return Err(ToolError::FileNotFound(search_path));
        }

        // 3. Build walker
        let mut builder = ignore::WalkBuilder::new(&search_path);
        builder
            .hidden(!params.show_hidden) // Filter hidden files unless requested
            .git_ignore(true) // Respect .gitignore
            .git_global(true)
            .git_exclude(true);

        // 4. Walk directory and collect matches
        let max_results = params.max_results.min(self.max_results);
        let working_dir = ctx.working_dir.clone();

        let search_future = tokio::task::spawn_blocking(move || {
            let walker = builder.build();
            let mut matches = Vec::new();
            let mut total_files = 0;

            for entry in walker {
                if matches.len() >= max_results {
                    break;
                }

                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                // Only match files, not directories
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    continue;
                }

                total_files += 1;
                let file_path = entry.path();

                // Get path relative to search root for matching
                let relative_path = file_path
                    .strip_prefix(&working_dir)
                    .unwrap_or(file_path);

                // Match against glob pattern
                if glob_matcher.is_match(relative_path) {
                    matches.push(relative_path.to_path_buf());
                }
            }

            // Sort matches alphabetically
            matches.sort();

            (matches, total_files)
        });

        let (mut matches, total_files) = tokio::time::timeout(self.timeout, search_future)
            .await
            .map_err(|_| ToolError::Timeout(self.timeout.as_millis() as u64))?
            .map_err(|e| ToolError::Other(e.into()))?;

        // 5. Format output
        let total_matches = matches.len();
        let shown_matches = matches.len().min(max_results);
        matches.truncate(shown_matches);

        let mut output = String::new();

        if shown_matches == 0 {
            output.push_str(&format!(
                "No files matching '{}' found\n",
                params.pattern
            ));
            output.push_str(&format!("Searched {} files total", total_files));
        } else {
            output.push_str(&format!(
                "Found {} files matching '{}':\n\n",
                shown_matches, params.pattern
            ));

            for file_path in &matches {
                output.push_str(&format!("{}\n", file_path.display()));
            }

            output.push('\n');

            if total_matches > shown_matches {
                output.push_str(&format!(
                    "(Showing {} of {} files. Use max_results to see more)\n",
                    shown_matches, total_matches
                ));
            } else {
                output.push_str(&format!("(Total {} files)\n", total_matches));
            }
        }

        tracing::debug!(
            total_matches,
            shown_matches,
            total_files,
            "tool glob done"
        );

        // 6. Return result
        Ok(ToolResult::new(
            format!("glob: {}", params.pattern),
            output,
        )
        .with_metadata("total_matches", json!(total_matches))
        .with_metadata("shown_matches", json!(shown_matches))
        .with_metadata("total_files", json!(total_files)))
    }
}
