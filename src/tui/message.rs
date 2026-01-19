use chrono::{DateTime, Local};

/// Type of error that occurred
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorType {
    Network,      // Network connection issues
    APIError,     // API returned an error response
    Timeout,      // Request timed out
    RateLimit,    // API rate limit exceeded
    InvalidInput, // Invalid user input
    ToolError,    // Tool execution failed
    Unknown,      // Unknown error type
}

impl ErrorType {
    /// Get a user-friendly suggestion for this error type
    pub fn suggestion(&self) -> &str {
        match self {
            ErrorType::Network => "Check your internet connection and try again",
            ErrorType::APIError => "Verify your API key in ~/.config/ok/config.toml",
            ErrorType::Timeout => "The request took too long. Try with a shorter input",
            ErrorType::RateLimit => "You've exceeded the API rate limit. Please wait a moment",
            ErrorType::InvalidInput => "Please check your input and try again",
            ErrorType::ToolError => "Tool execution failed. Check the error details above",
            ErrorType::Unknown => "An unexpected error occurred. Please try again",
        }
    }

    /// Get an icon for this error type
    pub fn icon(&self) -> &str {
        match self {
            ErrorType::Network => "🌐",
            ErrorType::APIError => "🔑",
            ErrorType::Timeout => "⏱️",
            ErrorType::RateLimit => "⚠️",
            ErrorType::InvalidInput => "📝",
            ErrorType::ToolError => "🔧",
            ErrorType::Unknown => "❌",
        }
    }
}

/// Detailed error information
#[derive(Debug, Clone)]
pub struct ErrorDetails {
    pub error_type: ErrorType,
    pub message: String,
    pub suggestion: String,
    pub timestamp: DateTime<Local>,
}

impl ErrorDetails {
    /// Create a new error with automatic suggestion
    pub fn new(error_type: ErrorType, message: String) -> Self {
        let suggestion = error_type.suggestion().to_string();
        Self {
            error_type,
            message,
            suggestion,
            timestamp: Local::now(),
        }
    }

    /// Create an error from a generic error message (tries to classify)
    pub fn from_message(message: String) -> Self {
        let error_type = Self::classify_error(&message);
        Self::new(error_type, message)
    }

    /// Classify error based on message content
    fn classify_error(message: &str) -> ErrorType {
        let lower = message.to_lowercase();
        if lower.contains("network") || lower.contains("connection") || lower.contains("dns") {
            ErrorType::Network
        } else if lower.contains("api key") || lower.contains("unauthorized") || lower.contains("401") {
            ErrorType::APIError
        } else if lower.contains("timeout") || lower.contains("timed out") {
            ErrorType::Timeout
        } else if lower.contains("rate limit") || lower.contains("429") {
            ErrorType::RateLimit
        } else if lower.contains("tool") {
            ErrorType::ToolError
        } else {
            ErrorType::Unknown
        }
    }

    /// Format the error for display
    pub fn format_for_display(&self) -> String {
        format!(
            "{} Error: {}\n\n💡 Suggestion: {}",
            self.error_type.icon(),
            self.message,
            self.suggestion
        )
    }
}

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

    /// Create an error message from ErrorDetails
    pub fn error_from_details(id: usize, details: ErrorDetails) -> Self {
        Self {
            id,
            role: MessageRole::Error,
            content: details.format_for_display(),
            timestamp: details.timestamp,
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
