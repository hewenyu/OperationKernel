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

async fn wait_for_line_counts(
    ctx: &ToolContext,
    shell_id: &str,
    min_stdout: usize,
    min_stderr: usize,
) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (stdout, stderr) = ctx
                .shell_manager
                .get_line_counts(shell_id)
                .await
                .unwrap_or((0, 0));
            if stdout >= min_stdout && stderr >= min_stderr {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timed out waiting for background shell output");
}

async fn wait_for_stdout_contains(ctx: &ToolContext, shell_id: &str, needle: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let stdout = ctx.shell_manager.get_stdout(shell_id).await.unwrap_or_default();
            if stdout.iter().any(|l| l.contains(needle)) {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timed out waiting for stdout to contain expected text");
}

async fn wait_for_stderr_contains(ctx: &ToolContext, shell_id: &str, needle: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let stderr = ctx.shell_manager.get_stderr(shell_id).await.unwrap_or_default();
            if stderr.iter().any(|l| l.contains(needle)) {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timed out waiting for stderr to contain expected text");
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

    wait_for_stdout_contains(&ctx, &shell_id, "line1").await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("line1"));
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

    wait_for_stdout_contains(&ctx, &shell_id, "completed").await;

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

    wait_for_line_counts(&ctx, &shell_id, 3, 0).await;

    let tool = BashOutputTool;

    // First call - get all lines
    let params1 = json!({
        "shell_id": shell_id,
        "offset": 0
    });

    let result1 = tool.execute(params1, &ctx).await;
    assert!(result1.is_ok());

    let output1 = result1.unwrap();
    assert_eq!(output1.metadata.get("new_stdout_lines"), Some(&json!(3)));
    let new_offset = output1.metadata.get("new_offset").unwrap().as_u64().unwrap() as usize;
    assert_eq!(new_offset, 3);

    // Second call - with offset (should return no new output)
    let params2 = json!({
        "shell_id": shell_id,
        "offset": new_offset
    });

    let result2 = tool.execute(params2, &ctx).await;
    assert!(result2.is_ok());

    let output2 = result2.unwrap();
    assert!(output2.output.contains("No new output") || output2.output.contains("(No new output since offset)"));
    assert_eq!(output2.metadata.get("new_stdout_lines"), Some(&json!(0)));
    assert_eq!(output2.metadata.get("new_stderr_lines"), Some(&json!(0)));
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

    wait_for_stderr_contains(&ctx, &shell_id, "stderr message").await;

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

    wait_for_line_counts(&ctx, &shell_id, 1, 0).await;

    let tool = BashOutputTool;

    // First call to get total lines
    let result1 = tool
        .execute(json!({"shell_id": shell_id}), &ctx)
        .await
        .unwrap();

    let total_lines = result1.metadata.get("new_offset").unwrap().as_u64().unwrap() as usize;
    assert_eq!(total_lines, 1);

    // Second call with offset matching total
    let params = json!({
        "shell_id": shell_id,
        "offset": total_lines
    });

    let result2 = tool.execute(params, &ctx).await;
    assert!(result2.is_ok());

    let output2 = result2.unwrap();
    assert!(output2.output.contains("No new output") || output2.output.contains("(No new output since offset)"));
    assert_eq!(
        output2.metadata.get("new_stdout_lines"),
        Some(&json!(0))
    );
    assert_eq!(
        output2.metadata.get("new_stderr_lines"),
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

    wait_for_line_counts(&ctx, &shell_id, 3, 0).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id,
        "filter": "^INFO:"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Should contain INFO lines
    assert!(output.output.contains("INFO: message1"));
    assert!(output.output.contains("INFO: message3"));
    assert!(!output.output.contains("ERROR: message2"));
    assert_eq!(output.metadata.get("new_stdout_lines"), Some(&json!(2)));
    assert_eq!(output.metadata.get("new_stderr_lines"), Some(&json!(0)));
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

    wait_for_line_counts(&ctx, &shell_id, 0, 2).await;

    let tool = BashOutputTool;
    let params = json!({
        "shell_id": shell_id,
        "filter": "ERROR"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    // The filter should only show lines containing ERROR
    let output = result.unwrap();
    assert!(output.output.contains("ERROR: error"));
    assert!(!output.output.contains("WARN: warning"));
    assert_eq!(output.metadata.get("new_stdout_lines"), Some(&json!(0)));
    assert_eq!(output.metadata.get("new_stderr_lines"), Some(&json!(1)));
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

    wait_for_line_counts(&ctx, &shell_id, 2, 1).await;

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
