//! Integration tests for the AskUserQuestion tool

mod common;

use common::TestFixture;
use ok::tool::{ask_user_question::AskUserQuestionTool, base::*};
use serde_json::json;
use std::sync::Arc;

/// Helper to create a tool context for testing
fn create_test_context(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext::new(
        "test_session",
        "test_msg_001",
        "test_agent",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

#[tokio::test]
async fn test_single_question_phase_1() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

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
                        "description": "Widely used"
                    }
                ],
                "multi_select": false
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok(), "Phase 1 should succeed");

    let output = result.unwrap();
    assert_eq!(output.title, "Awaiting user response");
    assert!(output.output.contains("PENDING"));
    assert_eq!(
        output.metadata.get("status").unwrap(),
        &json!("pending")
    );
    assert_eq!(
        output.metadata.get("num_questions").unwrap(),
        &json!(1)
    );
}

#[tokio::test]
async fn test_multiple_questions_phase_1() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "questions": [
            {
                "question": "Which database?",
                "header": "DB",
                "options": [
                    {"label": "PostgreSQL", "description": "Reliable"},
                    {"label": "MySQL", "description": "Popular"}
                ]
            },
            {
                "question": "Which ORM?",
                "header": "ORM",
                "options": [
                    {"label": "SQLAlchemy", "description": "Full-featured"},
                    {"label": "Diesel", "description": "Type-safe"}
                ]
            },
            {
                "question": "Which cache?",
                "header": "Cache",
                "options": [
                    {"label": "Redis", "description": "Fast"},
                    {"label": "Memcached", "description": "Simple"}
                ]
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(
        output.metadata.get("num_questions").unwrap(),
        &json!(3)
    );
}

#[tokio::test]
async fn test_phase_2_with_answers() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "questions": [
            {
                "question": "Which database?",
                "header": "DB",
                "options": [
                    {"label": "PostgreSQL", "description": "Reliable"},
                    {"label": "MySQL", "description": "Popular"}
                ]
            }
        ],
        "answers": {
            "q0": "PostgreSQL"
        }
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok(), "Phase 2 should succeed with answers");

    let output = result.unwrap();
    assert_eq!(output.title, "User questions answered");
    assert!(output.output.contains("PostgreSQL"));
    assert!(output.output.contains("User responses:"));
    assert_eq!(
        output.metadata.get("num_answers").unwrap(),
        &json!(1)
    );
}

#[tokio::test]
async fn test_multi_select_question() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "questions": [
            {
                "question": "Which features?",
                "header": "Features",
                "options": [
                    {"label": "Auth", "description": "Authentication"},
                    {"label": "API", "description": "REST API"},
                    {"label": "Cache", "description": "Caching"}
                ],
                "multi_select": true
            }
        ],
        "answers": {
            "q0": "Auth, API"
        }
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Auth, API"));
}

#[tokio::test]
async fn test_too_many_questions_error() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    // Try to create 5 questions (max is 4)
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
    assert!(result.is_err(), "Should fail with too many questions");

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("between 1 and 4"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_too_few_options_error() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "questions": [
            {
                "question": "Choose one?",
                "header": "Choice",
                "options": [
                    {"label": "Only one", "description": "Not enough options"}
                ]
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "Should fail with too few options");

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("2-4 options"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_header_too_long_error() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "questions": [
            {
                "question": "Test?",
                "header": "This header is way too long",
                "options": [
                    {"label": "A", "description": "D"},
                    {"label": "B", "description": "D"}
                ]
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "Should fail with header too long");

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("≤12 chars"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_empty_questions_array_error() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "questions": []
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "Should fail with empty questions");
}

#[tokio::test]
async fn test_max_valid_configuration() {
    let fixture = TestFixture::new();
    let tool = AskUserQuestionTool;
    let ctx = create_test_context(fixture.path());

    // Test maximum valid configuration: 4 questions, each with 4 options
    let params = json!({
        "questions": [
            {
                "question": "Q1?",
                "header": "12chars_max",
                "options": [
                    {"label": "O1", "description": "Option 1"},
                    {"label": "O2", "description": "Option 2"},
                    {"label": "O3", "description": "Option 3"},
                    {"label": "O4", "description": "Option 4"}
                ]
            },
            {
                "question": "Q2?",
                "header": "Header2",
                "options": [
                    {"label": "A", "description": "A"},
                    {"label": "B", "description": "B"},
                    {"label": "C", "description": "C"},
                    {"label": "D", "description": "D"}
                ]
            },
            {
                "question": "Q3?",
                "header": "H3",
                "options": [
                    {"label": "X", "description": "X"},
                    {"label": "Y", "description": "Y"},
                    {"label": "Z", "description": "Z"},
                    {"label": "W", "description": "W"}
                ]
            },
            {
                "question": "Q4?",
                "header": "Last",
                "options": [
                    {"label": "1", "description": "1"},
                    {"label": "2", "description": "2"},
                    {"label": "3", "description": "3"},
                    {"label": "4", "description": "4"}
                ]
            }
        ]
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok(), "Max valid config should succeed");

    let output = result.unwrap();
    assert_eq!(
        output.metadata.get("num_questions").unwrap(),
        &json!(4)
    );
}
