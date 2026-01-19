//! Integration tests for the bash tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, bash::BashTool};
use serde_json::json;

/// Helper to create a tool context for testing
fn create_test_context(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext::new("test_session", "test_msg", "test_station", working_dir)
}

#[tokio::test]
async fn test_bash_simple_echo() {
    let fixture = TestFixture::new();
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "echo 'Hello, World!'"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("Hello, World!"));
    assert_eq!(output.metadata.get("exit_code"), Some(&json!(0)));
}

#[tokio::test]
async fn test_bash_command_with_stderr() {
    let fixture = TestFixture::new();
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "echo 'stdout message' && echo 'stderr message' >&2"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("stdout message"));
    assert!(output.output.contains("stderr message"));
    assert!(output.output.contains("--- STDERR ---"));
}

#[tokio::test]
async fn test_bash_failing_command() {
    let fixture = TestFixture::new();
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "exit 1"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.get("exit_code"), Some(&json!(1)));
    assert_eq!(output.metadata.get("success"), Some(&json!(false)));
}

#[tokio::test]
async fn test_bash_working_directory() {
    let fixture = TestFixture::new();
    fixture.create_file("marker.txt", "test");

    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "ls -1"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("marker.txt"));
}

#[tokio::test]
async fn test_bash_timeout() {
    let fixture = TestFixture::new();
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "sleep 10",
        "timeout": 100  // 100ms timeout
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::Timeout(ms) => assert_eq!(ms, 100),
        _ => panic!("Expected timeout error"),
    }
}

#[tokio::test]
async fn test_bash_with_description() {
    let fixture = TestFixture::new();
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "echo test",
        "description": "Test echo command"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.title, "Test echo command");
}

#[tokio::test]
async fn test_bash_validation_rejects_find_root() {
    let fixture = TestFixture::new();
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "find / -name main.rs -type f 2>/dev/null | head -5"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("root directory '/'"));
            assert!(msg.contains("find ."));
            assert!(msg.contains("working directory"));
        }
        _ => panic!("Expected InvalidParams error for find / command"),
    }
}

#[tokio::test]
async fn test_bash_validation_allows_find_dot() {
    let fixture = TestFixture::new();
    fixture.create_file("test.txt", "content");
    let tool = BashTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "command": "find . -name '*.txt' -type f"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("test.txt"));
}

// TODO: Add more edge case tests
// - Test with special characters in command
// - Test with environment variables
// - Test with piped commands
// - Test with very long output
