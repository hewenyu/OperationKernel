use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Tool execution context - provides environment information to tools
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub message_id: String,
    pub agent: String,
    pub working_dir: PathBuf,
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
