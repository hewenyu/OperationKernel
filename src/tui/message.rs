use chrono::{DateTime, Local};

/// Represents the role/sender of a message
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Error,
}

/// Represents a single chat message in the conversation
#[derive(Debug, Clone)]
pub struct ChatMessage {
    #[allow(dead_code)]
    pub id: usize,
    pub role: MessageRole,
    pub content: String,
    #[allow(dead_code)]
    pub timestamp: DateTime<Local>,
    pub is_complete: bool,  // false indicates streaming in progress
}

impl ChatMessage {
    /// Create a new user message (always complete)
    pub fn user(id: usize, content: String) -> Self {
        Self {
            id,
            role: MessageRole::User,
            content,
            timestamp: Local::now(),
            is_complete: true,
        }
    }

    /// Create a new assistant message that's being streamed
    pub fn assistant_streaming(id: usize) -> Self {
        Self {
            id,
            role: MessageRole::Assistant,
            content: String::new(),
            timestamp: Local::now(),
            is_complete: false,
        }
    }

    /// Create a system message
    pub fn system(id: usize, content: String) -> Self {
        Self {
            id,
            role: MessageRole::System,
            content,
            timestamp: Local::now(),
            is_complete: true,
        }
    }

    /// Create an error message
    pub fn error(id: usize, content: String) -> Self {
        Self {
            id,
            role: MessageRole::Error,
            content,
            timestamp: Local::now(),
            is_complete: true,
        }
    }

    /// Append text to the message content (for streaming)
    pub fn append_content(&mut self, text: &str) {
        self.content.push_str(text);
    }

    /// Mark the message as complete (streaming finished)
    pub fn complete(&mut self) {
        self.is_complete = true;
    }

    /// Get formatted timestamp
    #[allow(dead_code)]
    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%H:%M:%S").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message() {
        let msg = ChatMessage::user(1, "Hello".to_string());
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");
        assert!(msg.is_complete);
    }

    #[test]
    fn test_streaming_message() {
        let mut msg = ChatMessage::assistant_streaming(2);
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content, "");
        assert!(!msg.is_complete);

        msg.append_content("Hello");
        msg.append_content(" world");
        assert_eq!(msg.content, "Hello world");
        assert!(!msg.is_complete);

        msg.complete();
        assert!(msg.is_complete);
    }
}
