use serde::{Deserialize, Serialize};

/// Message role in a conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Tool use request from Claude (in assistant message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub input: serde_json::Value,
}

/// Tool execution result (in user message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultContent {
    pub tool_use_id: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content block - supports text, tool use, and tool results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse(ToolUse),
    ToolResult(ToolResultContent),
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

/// Message content - can be simple string or array of blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(content.into()),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(content.into()),
        }
    }

    pub fn user_with_tool_result(tool_use_id: String, result: String) -> Self {
        Self::user_with_tool_result_detailed(tool_use_id, result, None)
    }

    pub fn user_with_tool_result_detailed(
        tool_use_id: String,
        result: String,
        is_error: Option<bool>,
    ) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult(
                ToolResultContent {
                    tool_use_id,
                    content: result,
                    is_error,
                },
            )]),
        }
    }

    pub fn assistant_with_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Blocks(blocks),
        }
    }
}

/// A chunk of streamed response
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// Text content delta
    Text(String),
    /// Tool use request
    ToolUse(ToolUse),
    /// Stream finished
    Done,
    /// Error occurred
    Error(String),
}
