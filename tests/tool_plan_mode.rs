//! Integration tests for EnterPlanMode and ExitPlanMode tools

mod common;

use common::TestFixture;
use ok::tool::{
    base::*, enter_plan_mode::EnterPlanModeTool, exit_plan_mode::ExitPlanModeTool,
};
use serde_json::json;
use std::sync::Arc;

/// Helper to create a tool context for testing
fn create_test_context(working_dir: std::path::PathBuf, session_id: &str) -> ToolContext {
    ToolContext::new(
        session_id,
        "test_msg_001",
        "test_agent",
        working_dir,
        Arc::new(ok::process::BackgroundShellManager::new()),
    )
}

// ============================================================================
// EnterPlanMode Tests
// ============================================================================

#[tokio::test]
async fn test_enter_plan_mode_success() {
    let fixture = TestFixture::new();
    let tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), "plan_session_1");

    let params = json!({});
    let result = tool.execute(params, &ctx).await;

    assert!(result.is_ok(), "EnterPlanMode should succeed");

    let output = result.unwrap();
    assert_eq!(output.title, "Entered plan mode");
    assert!(output.output.contains("plan mode"));
    assert!(output.output.contains("Explore the codebase"));
    assert_eq!(
        output.metadata.get("status").unwrap(),
        &json!("active")
    );
}

#[tokio::test]
async fn test_enter_plan_mode_creates_plan_file() {
    let fixture = TestFixture::new();
    let tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), "plan_session_2");

    let params = json!({});
    tool.execute(params, &ctx).await.unwrap();

    // Verify plan file was created
    let plan_file = fixture.path().join(".claude/plan-plan_session_2.md");
    assert!(
        plan_file.exists(),
        "Plan file should be created at: {}",
        plan_file.display()
    );

    // Verify plan file has template content
    let content = std::fs::read_to_string(&plan_file).unwrap();
    assert!(content.contains("# Implementation Plan"));
    assert!(content.contains("## Task Overview"));
    assert!(content.contains("## Step-by-Step Plan"));
    assert!(content.contains("## Files to Create/Modify"));
}

#[tokio::test]
async fn test_enter_plan_mode_creates_claude_directory() {
    let fixture = TestFixture::new();
    let tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), "plan_session_3");

    let claude_dir = fixture.path().join(".claude");
    assert!(
        !claude_dir.exists(),
        ".claude dir should not exist before test"
    );

    let params = json!({});
    tool.execute(params, &ctx).await.unwrap();

    assert!(
        claude_dir.exists(),
        ".claude directory should be created"
    );
    assert!(claude_dir.is_dir(), ".claude should be a directory");
}

#[tokio::test]
async fn test_enter_plan_mode_multiple_sessions() {
    let fixture = TestFixture::new();
    let tool = EnterPlanModeTool;

    // Create multiple plan sessions
    let ctx1 = create_test_context(fixture.path(), "session_a");
    let ctx2 = create_test_context(fixture.path(), "session_b");

    tool.execute(json!({}), &ctx1).await.unwrap();
    tool.execute(json!({}), &ctx2).await.unwrap();

    // Verify both plan files exist
    assert!(fixture
        .path()
        .join(".claude/plan-session_a.md")
        .exists());
    assert!(fixture
        .path()
        .join(".claude/plan-session_b.md")
        .exists());
}

// ============================================================================
// ExitPlanMode Tests
// ============================================================================

#[tokio::test]
async fn test_exit_plan_mode_no_plan_file_error() {
    let fixture = TestFixture::new();
    let tool = ExitPlanModeTool;
    let ctx = create_test_context(fixture.path(), "no_plan_session");

    let params = json!({});
    let result = tool.execute(params, &ctx).await;

    assert!(result.is_err(), "Should fail without plan file");

    match result.unwrap_err() {
        ToolError::InvalidParams(msg) => {
            assert!(msg.contains("Plan file not found"));
        }
        _ => panic!("Expected InvalidParams error"),
    }
}

#[tokio::test]
async fn test_exit_plan_mode_phase_1_awaiting() {
    let fixture = TestFixture::new();
    let session_id = "exit_test_1";

    // First enter plan mode
    let enter_tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), session_id);
    enter_tool.execute(json!({}), &ctx).await.unwrap();

    // Now exit plan mode (phase 1 - no approval yet)
    let exit_tool = ExitPlanModeTool;
    let params = json!({});
    let result = exit_tool.execute(params, &ctx).await;

    assert!(result.is_ok(), "Phase 1 should succeed");

    let output = result.unwrap();
    assert_eq!(output.title, "Awaiting plan approval");
    assert!(output.output.contains("PENDING"));
    assert_eq!(
        output.metadata.get("status").unwrap(),
        &json!("pending")
    );
}

#[tokio::test]
async fn test_exit_plan_mode_phase_2_approved() {
    let fixture = TestFixture::new();
    let session_id = "exit_test_2";

    // Enter plan mode first
    let enter_tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), session_id);
    enter_tool.execute(json!({}), &ctx).await.unwrap();

    // Exit with approval
    let exit_tool = ExitPlanModeTool;
    let params = json!({ "approved": true });
    let result = exit_tool.execute(params, &ctx).await;

    assert!(result.is_ok(), "Approval should succeed");

    let output = result.unwrap();
    assert_eq!(output.title, "Plan approved");
    assert!(output.output.contains("approved"));
    assert!(output.output.contains("All tools are now available"));
    assert_eq!(
        output.metadata.get("status").unwrap(),
        &json!("approved")
    );
}

#[tokio::test]
async fn test_exit_plan_mode_phase_2_rejected() {
    let fixture = TestFixture::new();
    let session_id = "exit_test_3";

    // Enter plan mode first
    let enter_tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), session_id);
    enter_tool.execute(json!({}), &ctx).await.unwrap();

    // Exit with rejection
    let exit_tool = ExitPlanModeTool;
    let params = json!({ "approved": false });
    let result = exit_tool.execute(params, &ctx).await;

    assert!(result.is_ok(), "Rejection should succeed");

    let output = result.unwrap();
    assert_eq!(output.title, "Plan rejected");
    assert!(output.output.contains("rejected"));
    assert!(output.output.contains("still in plan mode"));
    assert_eq!(
        output.metadata.get("status").unwrap(),
        &json!("rejected")
    );
}

#[tokio::test]
async fn test_exit_plan_mode_reads_custom_plan_content() {
    let fixture = TestFixture::new();
    let session_id = "exit_test_4";

    // Manually create plan file with custom content
    let claude_dir = fixture.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();

    let plan_file = claude_dir.join(format!("plan-{}.md", session_id));
    let custom_content = "# My Custom Plan\n\nThis is a detailed implementation plan.\n\n## Step 1\nDo something\n\n## Step 2\nDo something else";
    std::fs::write(&plan_file, custom_content).unwrap();

    // Exit plan mode (phase 1)
    let exit_tool = ExitPlanModeTool;
    let ctx = create_test_context(fixture.path(), session_id);
    let result = exit_tool.execute(json!({}), &ctx).await.unwrap();

    // Should contain plan length metadata
    let plan_length = result
        .metadata
        .get("plan_length")
        .unwrap()
        .as_u64()
        .unwrap();
    assert_eq!(plan_length, custom_content.len() as u64);
}

// ============================================================================
// Workflow Integration Tests
// ============================================================================

#[tokio::test]
async fn test_complete_plan_workflow_approved() {
    let fixture = TestFixture::new();
    let session_id = "workflow_test_1";

    // Step 1: Enter plan mode
    let enter_tool = EnterPlanModeTool;
    let ctx = create_test_context(fixture.path(), session_id);
    let enter_result = enter_tool.execute(json!({}), &ctx).await;
    assert!(enter_result.is_ok(), "Should enter plan mode");

    // Verify plan file exists
    let plan_file = fixture
        .path()
        .join(format!(".claude/plan-{}.md", session_id));
    assert!(plan_file.exists(), "Plan file should exist");

    // Step 2: Modify the plan (simulate agent writing to it)
    let updated_plan = "# Updated Plan\n\n## Implementation\n1. Create module\n2. Add tests\n3. Deploy";
    std::fs::write(&plan_file, updated_plan).unwrap();

    // Step 3: Exit plan mode - phase 1 (request approval)
    let exit_tool = ExitPlanModeTool;
    let exit_result_1 = exit_tool.execute(json!({}), &ctx).await;
    assert!(exit_result_1.is_ok(), "Phase 1 should succeed");
    let output_1 = exit_result_1.unwrap();
    assert_eq!(output_1.metadata.get("status").unwrap(), &json!("pending"));

    // Step 4: Exit plan mode - phase 2 (approval)
    let exit_result_2 = exit_tool.execute(json!({"approved": true}), &ctx).await;
    assert!(exit_result_2.is_ok(), "Phase 2 approval should succeed");
    let output_2 = exit_result_2.unwrap();
    assert_eq!(
        output_2.metadata.get("status").unwrap(),
        &json!("approved")
    );
}

#[tokio::test]
async fn test_complete_plan_workflow_rejected_and_revised() {
    let fixture = TestFixture::new();
    let session_id = "workflow_test_2";

    let ctx = create_test_context(fixture.path(), session_id);

    // Step 1: Enter plan mode
    let enter_tool = EnterPlanModeTool;
    enter_tool.execute(json!({}), &ctx).await.unwrap();

    // Step 2: Write initial plan
    let plan_file = fixture
        .path()
        .join(format!(".claude/plan-{}.md", session_id));
    std::fs::write(&plan_file, "# Initial Plan\nNot good enough").unwrap();

    // Step 3: Request approval
    let exit_tool = ExitPlanModeTool;
    exit_tool.execute(json!({}), &ctx).await.unwrap();

    // Step 4: User rejects
    let reject_result = exit_tool
        .execute(json!({"approved": false}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        reject_result.metadata.get("status").unwrap(),
        &json!("rejected")
    );

    // Step 5: Agent revises plan (still in plan mode)
    std::fs::write(&plan_file, "# Revised Plan\nMuch better now!").unwrap();

    // Step 6: Request approval again
    exit_tool.execute(json!({}), &ctx).await.unwrap();

    // Step 7: User approves revised plan
    let approve_result = exit_tool
        .execute(json!({"approved": true}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        approve_result.metadata.get("status").unwrap(),
        &json!("approved")
    );
}

#[tokio::test]
async fn test_plan_file_naming_with_special_session_ids() {
    let fixture = TestFixture::new();

    // Test various session ID formats
    let session_ids = vec![
        "simple",
        "with-dashes",
        "with_underscores",
        "123numeric",
        "UUID-abc123-def456",
    ];

    let enter_tool = EnterPlanModeTool;

    for session_id in session_ids {
        let ctx = create_test_context(fixture.path(), session_id);
        let result = enter_tool.execute(json!({}), &ctx).await;

        assert!(
            result.is_ok(),
            "Should handle session_id: {}",
            session_id
        );

        let plan_file = fixture
            .path()
            .join(format!(".claude/plan-{}.md", session_id));
        assert!(
            plan_file.exists(),
            "Plan file should exist for session: {}",
            session_id
        );
    }
}
