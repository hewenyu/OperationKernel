use super::base::{Tool, ToolContext, ToolError, ToolResult};
use crate::agent::{Question, QuestionOption};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

/// AskUserQuestion tool - allows the agent to ask the user questions interactively
///
/// This tool implements a two-phase execution model:
/// 1. First call (no answers): Returns PENDING and triggers UserQuestionRequest event
/// 2. Second call (with answers): Returns the final result with user selections
pub struct AskUserQuestionTool;

/// Input parameters for AskUserQuestion
#[derive(Debug, Deserialize)]
struct AskUserParams {
    questions: Vec<QuestionInput>,
    #[serde(default)]
    answers: Option<HashMap<String, String>>,
}

/// Question input from the agent
#[derive(Debug, Deserialize)]
struct QuestionInput {
    question: String,
    header: String,
    options: Vec<OptionInput>,
    #[serde(default)]
    multi_select: bool,
}

/// Option input for a question
#[derive(Debug, Deserialize)]
struct OptionInput {
    label: String,
    description: String,
}

impl QuestionInput {
    /// Convert to internal Question type
    fn to_question(&self) -> Question {
        Question {
            question: self.question.clone(),
            header: self.header.clone(),
            options: self
                .options
                .iter()
                .map(|opt| QuestionOption {
                    label: opt.label.clone(),
                    description: opt.description.clone(),
                })
                .collect(),
            multi_select: self.multi_select,
        }
    }
}

#[async_trait::async_trait]
impl Tool for AskUserQuestionTool {
    fn id(&self) -> &str {
        "ask_user_question"
    }

    fn description(&self) -> &str {
        "Ask the user questions during execution to gather preferences, clarify requirements, \
         or get decisions. Supports single-select and multi-select questions with customizable \
         options. Users can always select 'Other' to provide custom text input."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "List of questions to ask the user (1-4 questions)",
                    "minItems": 1,
                    "maxItems": 4,
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask (should end with a question mark)"
                            },
                            "header": {
                                "type": "string",
                                "description": "Short label displayed as a chip/tag (max 12 chars). Examples: 'Auth method', 'Library', 'Approach'"
                            },
                            "options": {
                                "type": "array",
                                "description": "Available choices for this question (2-4 options). 'Other' option is automatically added.",
                                "minItems": 2,
                                "maxItems": 4,
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Display text for this option (1-5 words)"
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means or what will happen"
                                        }
                                    },
                                    "required": ["label", "description"]
                                }
                            },
                            "multi_select": {
                                "type": "boolean",
                                "description": "Allow multiple options to be selected (default: false)",
                                "default": false
                            }
                        },
                        "required": ["question", "header", "options"]
                    }
                },
                "answers": {
                    "type": "object",
                    "description": "User answers (automatically provided on second call, do not set manually)",
                    "additionalProperties": {
                        "type": "string"
                    }
                }
            },
            "required": ["questions"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: AskUserParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::debug!(
            session_id = %ctx.session_id,
            num_questions = params.questions.len(),
            has_answers = params.answers.is_some(),
            "tool ask_user_question start"
        );

        // Validate number of questions
        if params.questions.is_empty() || params.questions.len() > 4 {
            return Err(ToolError::InvalidParams(format!(
                "Number of questions must be between 1 and 4, got {}",
                params.questions.len()
            )));
        }

        // Validate each question
        for (idx, q) in params.questions.iter().enumerate() {
            if q.options.len() < 2 || q.options.len() > 4 {
                return Err(ToolError::InvalidParams(format!(
                    "Question {} must have 2-4 options, got {}",
                    idx + 1,
                    q.options.len()
                )));
            }
            if q.header.len() > 12 {
                return Err(ToolError::InvalidParams(format!(
                    "Question {} header must be ≤12 chars, got {} chars",
                    idx + 1,
                    q.header.len()
                )));
            }
        }

        // Phase 1: No answers provided - trigger UI interaction
        if params.answers.is_none() {
            tracing::info!(
                tool_use_id = %ctx.message_id,
                "ask_user_question: awaiting user response"
            );

            // Convert questions to internal format
            let questions: Vec<Question> = params
                .questions
                .iter()
                .map(|q| q.to_question())
                .collect();

            // This would trigger a UserQuestionRequest event in the agent runner
            // For now, return a pending result
            return Ok(ToolResult::new(
                "Awaiting user response",
                "PENDING: User input requested. Questions have been displayed to the user."
            )
            .with_metadata("status", json!("pending"))
            .with_metadata("num_questions", json!(questions.len())));
        }

        // Phase 2: Answers provided - return final result
        let answers = params.answers.unwrap();

        tracing::info!(
            tool_use_id = %ctx.message_id,
            num_answers = answers.len(),
            "ask_user_question: user response received"
        );

        // Format answers for output
        let mut output = String::from("User responses:\n\n");
        for (idx, q) in params.questions.iter().enumerate() {
            let question_key = format!("q{}", idx);
            if let Some(answer) = answers.get(&question_key) {
                output.push_str(&format!("Q: {}\n", q.question));
                output.push_str(&format!("A: {}\n\n", answer));
            }
        }

        tracing::debug!(
            session_id = %ctx.session_id,
            "tool ask_user_question done"
        );

        Ok(ToolResult::new("User questions answered", output)
            .with_metadata("num_questions", json!(params.questions.len()))
            .with_metadata("num_answers", json!(answers.len())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_test_context() -> ToolContext {
        ToolContext {
            session_id: "test-session".to_string(),
            message_id: "test-message".to_string(),
            agent: "test-agent".to_string(),
            working_dir: PathBuf::from("/tmp"),
            shell_manager: Arc::new(crate::process::BackgroundShellManager::new()),
        }
    }

    #[tokio::test]
    async fn test_phase_1_no_answers() {
        let tool = AskUserQuestionTool;
        let ctx = create_test_context();

        let params = json!({
            "questions": [
                {
                    "question": "Which database should we use?",
                    "header": "Database",
                    "options": [
                        {
                            "label": "PostgreSQL",
                            "description": "Reliable and feature-rich"
                        },
                        {
                            "label": "MySQL",
                            "description": "Widely used and well-supported"
                        }
                    ],
                    "multi_select": false
                }
            ]
        });

        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "Awaiting user response");
        assert!(result.output.contains("PENDING"));
        assert_eq!(result.metadata.get("status").unwrap(), &json!("pending"));
    }

    #[tokio::test]
    async fn test_phase_2_with_answers() {
        let tool = AskUserQuestionTool;
        let ctx = create_test_context();

        let params = json!({
            "questions": [
                {
                    "question": "Which database should we use?",
                    "header": "Database",
                    "options": [
                        {
                            "label": "PostgreSQL",
                            "description": "Reliable and feature-rich"
                        },
                        {
                            "label": "MySQL",
                            "description": "Widely used and well-supported"
                        }
                    ],
                    "multi_select": false
                }
            ],
            "answers": {
                "q0": "PostgreSQL"
            }
        });

        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "User questions answered");
        assert!(result.output.contains("PostgreSQL"));
        assert_eq!(result.metadata.get("num_answers").unwrap(), &json!(1));
    }

    #[tokio::test]
    async fn test_too_many_questions() {
        let tool = AskUserQuestionTool;
        let ctx = create_test_context();

        let params = json!({
            "questions": [
                {"question": "Q1?", "header": "H1", "options": [{"label": "A", "description": "D"}, {"label": "B", "description": "D"}]},
                {"question": "Q2?", "header": "H2", "options": [{"label": "A", "description": "D"}, {"label": "B", "description": "D"}]},
                {"question": "Q3?", "header": "H3", "options": [{"label": "A", "description": "D"}, {"label": "B", "description": "D"}]},
                {"question": "Q4?", "header": "H4", "options": [{"label": "A", "description": "D"}, {"label": "B", "description": "D"}]},
                {"question": "Q5?", "header": "H5", "options": [{"label": "A", "description": "D"}, {"label": "B", "description": "D"}]}
            ]
        });

        let result = tool.execute(params, &ctx).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn test_invalid_header_length() {
        let tool = AskUserQuestionTool;
        let ctx = create_test_context();

        let params = json!({
            "questions": [
                {
                    "question": "Test?",
                    "header": "This is too long header",
                    "options": [
                        {"label": "A", "description": "D"},
                        {"label": "B", "description": "D"}
                    ]
                }
            ]
        });

        let result = tool.execute(params, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_select_question() {
        let tool = AskUserQuestionTool;
        let ctx = create_test_context();

        let params = json!({
            "questions": [
                {
                    "question": "Which features do you want?",
                    "header": "Features",
                    "options": [
                        {"label": "Auth", "description": "User authentication"},
                        {"label": "API", "description": "REST API"},
                        {"label": "Cache", "description": "Redis caching"}
                    ],
                    "multi_select": true
                }
            ],
            "answers": {
                "q0": "Auth, API"
            }
        });

        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "User questions answered");
        assert!(result.output.contains("Auth, API"));
    }
}
