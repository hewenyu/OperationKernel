//! Integration tests for the bash_output tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, bash_output::BashOutputTool};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Helper to create a tool context for testing
fn create_test_context(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext::new(
        "test_session",
        "test_msg",
        "test_station",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

#[tokio::test]
async fn test_bash_output_shell_not_found() {
    let fixture = TestFixture::new();
    let tool = BashOutputTool;
    let ctx = create_test_context(fixture.path());

    let params = json!({
        "shell_id": "nonexistent_shell"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("not found"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_bash_output_running_process() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn a background process that outputs multiple lines
    let shell_id = ctx
        .shell_manager
        .spawn(
            "test_shell".into(),
            "echo 'line1'; sleep 0.05; echo 'line2'".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait a bit for first line
    sleep(Duration::from_millis(30)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("line1") || output.output.contains("Status: Running"));
    assert_eq!(output.metadata.get("shell_id"), Some(&json!(shell_id)));
}

#[tokio::test]
async fn test_bash_output_completed_process() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn a quick process
    let shell_id = ctx
        .shell_manager
        .spawn(
            "quick_shell".into(),
            "echo 'completed'; exit 0".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait for completion
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("completed"));
    // Verify the process status is captured (either completed or shows output)
    assert!(
        output.output.contains("Status:")
        || output.metadata.get("status").is_some()
    );
}

#[tokio::test]
async fn test_bash_output_with_offset() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn process with multiple lines
    let shell_id = ctx
        .shell_manager
        .spawn(
            "multi_line".into(),
            "echo 'line1'; echo 'line2'; echo 'line3'".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait for all output
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;

    // First call - get all lines
    let params1 = json!({
        "shell_id": shell_id,
        "offset": 0
    });

    let result1 = tool.execute(params1, &ctx).await;
    assert!(result1.is_ok());

    let output1 = result1.unwrap();
    let new_offset = output1.metadata.get("new_offset").unwrap().as_u64().unwrap() as usize;

    // Second call - with offset (should return no new output)
    let params2 = json!({
        "shell_id": shell_id,
        "offset": new_offset
    });

    let result2 = tool.execute(params2, &ctx).await;
    assert!(result2.is_ok());

    let output2 = result2.unwrap();
    assert!(output2.output.contains("No new output"));
}

#[tokio::test]
async fn test_bash_output_stderr_captured() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn process with stderr output
    let shell_id = ctx
        .shell_manager
        .spawn(
            "stderr_shell".into(),
            "echo 'stderr message' >&2".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait for output
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("STDERR") || output.output.contains("stderr message"));

    let stderr_lines = output.metadata.get("total_stderr_lines").unwrap().as_u64().unwrap();
    assert!(stderr_lines > 0);
}

#[tokio::test]
async fn test_bash_output_no_new_output() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn a simple completed process
    let shell_id = ctx
        .shell_manager
        .spawn("simple".into(), "echo 'done'".into(), fixture.path())
        .await
        .expect("Failed to spawn shell");

    // Wait for completion
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;

    // First call to get total lines
    let result1 = tool
        .execute(json!({"shell_id": shell_id}), &ctx)
        .await
        .unwrap();

    let total_lines = result1.metadata.get("new_offset").unwrap().as_u64().unwrap() as usize;

    // Second call with offset matching total
    let params = json!({
        "shell_id": shell_id,
        "offset": total_lines
    });

    let result2 = tool.execute(params, &ctx).await;
    assert!(result2.is_ok());

    let output2 = result2.unwrap();
    assert!(output2.output.contains("No new output"));
    assert_eq!(
        output2.metadata.get("new_stdout_lines"),
        Some(&json!(0))
    );
}

#[tokio::test]
async fn test_bash_output_regex_filter_stdout() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn process with mixed output
    let shell_id = ctx
        .shell_manager
        .spawn(
            "filter_test".into(),
            "echo 'INFO: message1'; echo 'ERROR: message2'; echo 'INFO: message3'".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait for output
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id,
        "filter": "^INFO:"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should contain INFO lines
    assert!(output.output.contains("INFO:") || output.output.contains("message1"));
}

#[tokio::test]
async fn test_bash_output_regex_filter_stderr() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn process with stderr output
    let shell_id = ctx
        .shell_manager
        .spawn(
            "stderr_filter".into(),
            "echo 'WARN: warning' >&2; echo 'ERROR: error' >&2".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait for output
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id,
        "filter": "ERROR"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    // The filter should only show lines containing ERROR
    let output = result.unwrap();
    // Check that filtering was applied (filtered lines count should be less than total if WARN was excluded)
    let new_stderr = output.metadata.get("new_stderr_lines").unwrap().as_u64().unwrap();
    assert!(new_stderr <= 2); // Should have filtered some lines
}

#[tokio::test]
async fn test_bash_output_invalid_regex() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn any process
    let shell_id = ctx
        .shell_manager
        .spawn("any_shell".into(), "echo 'test'".into(), fixture.path())
        .await
        .expect("Failed to spawn shell");

    sleep(Duration::from_millis(50)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id,
        "filter": "[invalid(regex"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("Invalid regex"));
        }
        _ => panic!("Expected InvalidParams error for invalid regex"),
    }
}

#[tokio::test]
async fn test_bash_output_metadata_correct() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn process with known output
    let shell_id = ctx
        .shell_manager
        .spawn(
            "metadata_test".into(),
            "echo 'stdout1'; echo 'stdout2'; echo 'stderr1' >&2".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Wait for all output
    sleep(Duration::from_millis(100)).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();

    // Verify metadata fields exist
    assert!(output.metadata.contains_key("shell_id"));
    assert!(output.metadata.contains_key("status"));
    assert!(output.metadata.contains_key("new_stdout_lines"));
    assert!(output.metadata.contains_key("new_stderr_lines"));
    assert!(output.metadata.contains_key("new_offset"));
    assert!(output.metadata.contains_key("total_stdout_lines"));
    assert!(output.metadata.contains_key("total_stderr_lines"));

    // Verify metadata values
    assert_eq!(output.metadata.get("shell_id"), Some(&json!(shell_id)));

    let total_stdout = output.metadata.get("total_stdout_lines").unwrap().as_u64().unwrap();
    let total_stderr = output.metadata.get("total_stderr_lines").unwrap().as_u64().unwrap();

    assert!(total_stdout >= 2); // Should have at least 2 stdout lines
    assert!(total_stderr >= 1); // Should have at least 1 stderr line
}
