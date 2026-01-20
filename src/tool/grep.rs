use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Grep tool - searches file contents using regex patterns
pub struct GrepTool {
    max_results: usize,
    max_line_length: usize,
    timeout: Duration,
}

impl GrepTool {
    pub fn new() -> Self {
        Self {
            max_results: 100,
            max_line_length: 500,
            timeout: Duration::from_secs(10),
        }
    }

    /// Check if a file is likely binary by examining first 512 bytes
    fn is_binary_file(path: &Path) -> std::io::Result<bool> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut buffer = [0u8; 512];
        let n = file.read(&mut buffer)?;

        if n == 0 {
            return Ok(false); // Empty file is not binary
        }

        // Check for null bytes (strong indicator of binary)
        Ok(buffer[..n].contains(&0))
    }

    /// Search for pattern in a single file
    fn search_file(
        &self,
        file_path: &Path,
        pattern: &regex::Regex,
        context_lines: usize,
    ) -> anyhow::Result<Vec<Match>> {
        // Skip binary files
        if Self::is_binary_file(file_path).unwrap_or(false) {
            return Ok(Vec::new());
        }

        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()), // Skip unreadable files
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            if pattern.is_match(line) {
                // Collect context lines
                let start = line_idx.saturating_sub(context_lines);
                let end = (line_idx + context_lines + 1).min(lines.len());

                let mut context = Vec::new();
                for i in start..end {
                    let prefix = if i == line_idx { ">" } else { " " };
                    let line_num = i + 1; // 1-based line numbers

                    // Truncate long lines
                    let line_text = if lines[i].len() > self.max_line_length {
                        format!("{}...", &lines[i][..self.max_line_length])
                    } else {
                        lines[i].to_string()
                    };

                    context.push(ContextLine {
                        line_number: line_num,
                        content: line_text,
                        prefix: prefix.to_string(),
                    });
                }

                matches.push(Match {
                    file_path: file_path.to_path_buf(),
                    context,
                });
            }
        }

        Ok(matches)
    }
}

#[derive(Debug, Deserialize)]
struct GrepParams {
    pattern: String,
    #[serde(default = "default_path")]
    path: PathBuf,
    #[serde(default = "default_case_sensitive")]
    case_sensitive: bool,
    #[serde(default = "default_max_results")]
    max_results: usize,
    #[serde(default)]
    context_lines: usize,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    exclude_patterns: Vec<String>,
}

fn default_path() -> PathBuf {
    PathBuf::from(".")
}

fn default_case_sensitive() -> bool {
    true
}

fn default_max_results() -> usize {
    100
}

#[derive(Debug)]
struct Match {
    file_path: PathBuf,
    context: Vec<ContextLine>,
}

#[derive(Debug)]
struct ContextLine {
    line_number: usize,
    content: String,
    prefix: String,
}

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn id(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns. \
         Supports context lines, case sensitivity, include/exclude patterns. \
         Respects .gitignore files. Skips binary files automatically."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: current directory)",
                    "default": "."
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Whether to perform case-sensitive search (default: true)",
                    "default": true
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 100)",
                    "default": 100
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines to show before/after match (default: 0)",
                    "default": 0
                },
                "include_patterns": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Glob patterns for files to include (e.g., ['*.rs', '*.toml'])",
                    "default": []
                },
                "exclude_patterns": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Glob patterns for files to exclude",
                    "default": []
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
        let params: GrepParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            working_dir = %ctx.working_dir.display(),
            pattern = %params.pattern,
            path = %params.path.display(),
            case_sensitive = params.case_sensitive,
            max_results = params.max_results,
            "tool grep start"
        );

        // 1. Validate and compile regex
        let regex_pattern = if params.case_sensitive {
            params.pattern.clone()
        } else {
            format!("(?i){}", params.pattern)
        };

        let pattern = regex::Regex::new(&regex_pattern)
            .map_err(|e| ToolError::InvalidParams(format!("Invalid regex: {}", e)))?;

        // 2. Resolve search path
        let search_path = ctx.resolve_path(&params.path)?;

        if !search_path.exists() {
            return Err(ToolError::FileNotFound(search_path));
        }

        // 3. Build walker with ignore support
        let mut builder = ignore::WalkBuilder::new(&search_path);
        builder
            .hidden(false) // Include hidden files by default
            .git_ignore(true) // Respect .gitignore
            .git_global(true)
            .git_exclude(true);

        // Add include/exclude patterns (glob filters).
        // IMPORTANT: Overrides must be built once; multiple `builder.overrides(...)` calls will
        // replace the previous overrides and break include+exclude combination.
        if !params.include_patterns.is_empty() || !params.exclude_patterns.is_empty() {
            let mut override_builder = ignore::overrides::OverrideBuilder::new(&search_path);

            // Without '!' => whitelist (include). With '!' => ignore (exclude).
            for include in &params.include_patterns {
                override_builder
                    .add(include)
                    .map_err(|e| {
                        ToolError::InvalidParams(format!("Invalid include pattern: {}", e))
                    })?;
            }
            for exclude in &params.exclude_patterns {
                override_builder
                    .add(&format!("!{}", exclude))
                    .map_err(|e| {
                        ToolError::InvalidParams(format!("Invalid exclude pattern: {}", e))
                    })?;
            }

            let overrides = override_builder
                .build()
                .map_err(|e| {
                    ToolError::InvalidParams(format!("Failed to build overrides: {}", e))
                })?;
            builder.overrides(overrides);
        }

        // 4. Search files with timeout
        let max_results = params.max_results.min(self.max_results);
        let context_lines = params.context_lines;

        let search_future = tokio::task::spawn_blocking(move || {
            let walker = builder.build();
            let mut all_matches = Vec::new();
            let mut files_searched = 0;
            let mut binary_files_skipped = 0;

            for entry in walker {
                if all_matches.len() >= max_results {
                    break;
                }

                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                // Only search files, not directories
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    continue;
                }

                let file_path = entry.path();
                files_searched += 1;

                // Check if binary
                if Self::is_binary_file(file_path).unwrap_or(false) {
                    binary_files_skipped += 1;
                    continue;
                }

                // Search this file
                let tool = GrepTool::new();
                match tool.search_file(file_path, &pattern, context_lines) {
                    Ok(mut matches) => {
                        all_matches.append(&mut matches);
                    }
                    Err(_) => continue,
                }
            }

            (all_matches, files_searched, binary_files_skipped)
        });

        let (all_matches, files_searched, binary_files_skipped) =
            tokio::time::timeout(self.timeout, search_future)
                .await
                .map_err(|_| ToolError::Timeout(self.timeout.as_millis() as u64))?
                .map_err(|e| ToolError::Other(e.into()))?;

        // 5. Format output
        let total_matches = all_matches.len();
        let matches_to_show = all_matches.into_iter().take(max_results).collect::<Vec<_>>();
        let shown_matches = matches_to_show.len();

        let mut output = String::new();

        if shown_matches == 0 {
            output.push_str(&format!("No matches found for pattern: {}\n", params.pattern));
            output.push_str(&format!("Searched {} files", files_searched));
            if binary_files_skipped > 0 {
                output.push_str(&format!(" (skipped {} binary files)", binary_files_skipped));
            }
        } else {
            // Group matches by file
            let mut files_with_matches: std::collections::HashMap<PathBuf, Vec<&Match>> =
                std::collections::HashMap::new();
            for m in &matches_to_show {
                files_with_matches
                    .entry(m.file_path.clone())
                    .or_insert_with(Vec::new)
                    .push(m);
            }

            output.push_str(&format!(
                "Found {} matches in {} files:\n\n",
                shown_matches,
                files_with_matches.len()
            ));

            for (file_path, matches) in files_with_matches {
                let relative_path = file_path
                    .strip_prefix(&ctx.working_dir)
                    .unwrap_or(&file_path);

                output.push_str(&format!("{}:\n", relative_path.display()));

                for m in matches {
                    // Show context lines
                    for ctx_line in &m.context {
                        output.push_str(&format!(
                            "{} {:>4}\u{2502} {}\n",
                            ctx_line.prefix, ctx_line.line_number, ctx_line.content
                        ));
                    }
                    output.push('\n');
                }
            }

            if total_matches > shown_matches {
                output.push_str(&format!(
                    "(Showing {} of {} matches. Use max_results to see more)\n",
                    shown_matches, total_matches
                ));
            } else {
                output.push_str(&format!("(Total {} matches)\n", total_matches));
            }

            if binary_files_skipped > 0 {
                output.push_str(&format!("(Skipped {} binary files)\n", binary_files_skipped));
            }
        }

        tracing::debug!(
            total_matches,
            shown_matches,
            files_searched,
            binary_files_skipped,
            "tool grep done"
        );

        // 6. Return result
        Ok(ToolResult::new(
            format!("grep: {}", params.pattern),
            output,
        )
        .with_metadata("total_matches", json!(total_matches))
        .with_metadata("shown_matches", json!(shown_matches))
        .with_metadata("files_searched", json!(files_searched))
        .with_metadata("binary_files_skipped", json!(binary_files_skipped)))
    }
}
