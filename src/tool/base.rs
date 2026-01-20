use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::process::BackgroundShellManager;

/// Tool execution context - provides environment information to tools
#[derive(Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub message_id: String,
    pub agent: String,
    pub working_dir: PathBuf,
    pub shell_manager: Arc<BackgroundShellManager>,
}

fn lexical_normalize_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;

    let mut prefix: Option<std::ffi::OsString> = None;
    let mut is_absolute = false;
    let mut stack: Vec<std::ffi::OsString> = Vec::new();

    for component in path.components() {
        match component {
            Component::Prefix(p) => {
                prefix = Some(p.as_os_str().to_owned());
            }
            Component::RootDir => {
                is_absolute = true;
                stack.clear();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(last) = stack.last() {
                    if last != ".." {
                        stack.pop();
                    } else if !is_absolute {
                        stack.push("..".into());
                    }
                } else if !is_absolute {
                    stack.push("..".into());
                }
            }
            Component::Normal(s) => stack.push(s.to_owned()),
        }
    }

    let mut normalized = PathBuf::new();
    if let Some(p) = prefix {
        normalized.push(p);
    }
    if is_absolute {
        normalized.push(std::path::MAIN_SEPARATOR.to_string());
    }
    for part in stack {
        normalized.push(part);
    }
    normalized
}

impl ToolContext {
    /// Resolve a user-supplied path against `working_dir`.
    ///
    /// - Relative paths are joined to `working_dir`.
    /// - Absolute paths are allowed only if they are within `working_dir`.
    pub fn resolve_path(&self, input: &PathBuf) -> Result<PathBuf, ToolError> {
        let root = lexical_normalize_path(&self.working_dir);
        let resolved = if input.is_absolute() {
            lexical_normalize_path(input)
        } else {
            lexical_normalize_path(&root.join(input))
        };

        // Guardrail: keep tool operations scoped to working_dir unless explicitly changed.
        if resolved.is_absolute() && !resolved.starts_with(&root) {
            return Err(ToolError::InvalidParams(format!(
                "Path is outside working directory.\n\
                 Working directory: {}\n\
                 Requested path: {}\n\
                 Suggestion: use a relative path (e.g. './...') or change the working directory first.",
                root.display(),
                resolved.display()
            )));
        }

        Ok(resolved)
    }
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("session_id", &self.session_id)
            .field("message_id", &self.message_id)
            .field("agent", &self.agent)
            .field("working_dir", &self.working_dir)
            .field("shell_manager", &"<BackgroundShellManager>")
            .finish()
    }
}

/// Tool execution result returned to Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Human-readable title/summary
    pub title: String,
    /// Tool output content
    pub output: String,
    /// Additional metadata (exit codes, file info, etc.)
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolResult {
    pub fn new(title: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            output: output.into(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Tool execution errors
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Binary file detected: {0}")]
    BinaryFile(PathBuf),

    #[error("Command failed with exit code {code:?}: {message}")]
    CommandFailed { code: Option<i32>, message: String },

    #[error("Command timed out after {0}ms")]
    Timeout(u64),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    // Edit tool specific errors
    #[error("String not found in file: {0}")]
    OldStringNotFound(String),

    #[error("Multiple matches found ({count} occurrences at positions {positions:?}). Use replace_all=true or provide more context to make the match unique.")]
    MultipleMatches { count: usize, positions: Vec<usize> },

    #[error("old_string and new_string must be different")]
    OldNewIdentical,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Base tool trait - all tools must implement this
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool identifier (e.g., "bash", "read", "write")
    fn id(&self) -> &str;

    /// Human-readable description for Claude
    fn description(&self) -> &str;

    /// JSON schema for tool parameters
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with given parameters
    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError>;
}
