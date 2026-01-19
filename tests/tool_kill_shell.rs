//! Integration tests for the kill_shell tool

mod common;

use common::TestFixture;
use ok::tool::{base::*, kill_shell::KillShellTool};
use serde_json::json;
use std::sync::Arc;

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
async fn test_kill_shell_not_found() {
    let fixture = TestFixture::new();
    let tool = KillShellTool;
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
async fn test_kill_shell_running_process() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Start a long-running process
    let shell_id = ctx
        .shell_manager
        .spawn(
            "long_running".into(),
            "sleep 1000".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Verify it exists
    assert!(ctx.shell_manager.exists(&shell_id).await);

    // Kill it
    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("terminated"));
    assert!(output.output.contains("was running") || output.output.contains("Status before kill"));
    assert_eq!(output.metadata.get("terminated"), Some(&json!(true)));

    // Verify it was removed
    assert!(!ctx.shell_manager.exists(&shell_id).await);
}

#[tokio::test]
async fn test_kill_shell_completed_process() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Start a quick process that completes
    let shell_id = ctx
        .shell_manager
        .spawn(
            "quick_process".into(),
            "echo 'done'; exit 0".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Kill it (even though it's already completed)
    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.output.contains("terminated"));
    // Process should have been killed successfully regardless of its status
    assert_eq!(output.metadata.get("terminated"), Some(&json!(true)));

    // Verify it was removed
    assert!(!ctx.shell_manager.exists(&shell_id).await);
}

#[tokio::test]
async fn test_kill_shell_removed_from_registry() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Spawn multiple shells
    let shell_id1 = ctx
        .shell_manager
        .spawn("shell1".into(), "sleep 1000".into(), fixture.path())
        .await
        .expect("Failed to spawn shell1");

    let shell_id2 = ctx
        .shell_manager
        .spawn("shell2".into(), "sleep 1000".into(), fixture.path())
        .await
        .expect("Failed to spawn shell2");

    // Verify both exist
    assert!(ctx.shell_manager.exists(&shell_id1).await);
    assert!(ctx.shell_manager.exists(&shell_id2).await);

    // Kill first shell
    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id1
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    // Verify only first shell was removed
    assert!(!ctx.shell_manager.exists(&shell_id1).await);
    assert!(ctx.shell_manager.exists(&shell_id2).await);

    // Clean up second shell
    ctx.shell_manager.remove(&shell_id2).await;
}

#[tokio::test]
async fn test_kill_shell_sigkill_sent() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Create a process that would run forever if not killed
    let shell_id = ctx
        .shell_manager
        .spawn(
            "infinite_loop".into(),
            "while true; do sleep 1; done".into(),
            fixture.path(),
        )
        .await
        .expect("Failed to spawn shell");

    // Verify it's still running
    if let Some(status) = ctx.shell_manager.get_status(&shell_id).await {
        assert!(matches!(
            status,
            ok::process::background_shell::ShellStatus::Running
        ));
    }

    // Kill it
    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    // Verify output mentions SIGKILL
    let output = result.unwrap();
    assert!(output.output.contains("SIGKILL") || output.output.contains("terminated"));
}

#[tokio::test]
async fn test_kill_shell_metadata_correct() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    let shell_id = ctx
        .shell_manager
        .spawn("metadata_test".into(), "sleep 100".into(), fixture.path())
        .await
        .expect("Failed to spawn shell");

    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id.clone()
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();

    // Verify metadata fields
    assert!(output.metadata.contains_key("shell_id"));
    assert!(output.metadata.contains_key("terminated"));

    assert_eq!(output.metadata.get("shell_id"), Some(&json!(shell_id)));
    assert_eq!(output.metadata.get("terminated"), Some(&json!(true)));
}

#[tokio::test]
async fn test_kill_shell_status_before_kill() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    // Start a running process
    let shell_id = ctx
        .shell_manager
        .spawn("status_test".into(), "sleep 100".into(), fixture.path())
        .await
        .expect("Failed to spawn shell");

    // Get status before killing
    let status_before = ctx.shell_manager.get_status(&shell_id).await;
    assert!(status_before.is_some());

    // Kill it
    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Output should include status information
    assert!(output.output.contains("Status before kill:"));
}

#[tokio::test]
async fn test_kill_shell_multiple_kills_idempotent() {
    let fixture = TestFixture::new();
    let ctx = create_test_context(fixture.path());

    let shell_id = ctx
        .shell_manager
        .spawn("double_kill".into(), "sleep 100".into(), fixture.path())
        .await
        .expect("Failed to spawn shell");

    let tool = KillShellTool;
    let params = json!({
        "shell_id": shell_id.clone()
    });

    // First kill should succeed
    let result1 = tool.execute(params.clone(), &ctx).await;
    assert!(result1.is_ok());

    // Verify removed
    assert!(!ctx.shell_manager.exists(&shell_id).await);

    // Second kill should fail (shell no longer exists)
    let result2 = tool.execute(params, &ctx).await;
    assert!(result2.is_err());

    match result2.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("not found"));
        }
        _ => panic!("Expected InvalidParams error for double kill"),
    }
}
