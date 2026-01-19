use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;

/// ExitPlanMode tool - reads the plan file and requests user approval
///
/// This tool:
/// 1. Reads the plan file created during plan mode
/// 2. Triggers a PlanApprovalRequest event to display plan to user
/// 3. Waits for user approval or rejection
/// 4. On approval, exits plan mode and re-enables all tools
pub struct ExitPlanModeTool;

/// Input parameters for ExitPlanMode
#[derive(Debug, Deserialize)]
struct ExitPlanModeParams {
    /// Optional: User approval status (automatically provided on second call)
    #[serde(default)]
    approved: Option<bool>,
}

#[async_trait::async_trait]
impl Tool for ExitPlanModeTool {
    fn id(&self) -> &str {
        "exit_plan_mode"
    }

    fn description(&self) -> &str {
        "Exit plan mode and request user approval of the plan. \
         Reads the plan file and displays it to the user for review. \
         Only use this after you have finished writing your plan."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "approved": {
                    "type": "boolean",
                    "description": "User approval status (automatically provided, do not set manually)"
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: ExitPlanModeParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        // Determine plan file path
        let plan_file = ctx
            .working_dir
            .join(".claude")
            .join(format!("plan-{}.md", ctx.session_id));

        // Check if plan file exists
        if !plan_file.exists() {
            return Err(ToolError::InvalidParams(format!(
                "Plan file not found: {}. Did you enter plan mode first?",
                plan_file.display()
            )));
        }

        // Read plan content
        let plan_content = tokio::fs::read_to_string(&plan_file)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        // Phase 1: No approval decision yet - trigger UI interaction
        if params.approved.is_none() {
            tracing::info!(
                session_id = %ctx.session_id,
                plan_file = %plan_file.display(),
                "tool exit_plan_mode: awaiting user approval"
            );

            // This would trigger a PlanApprovalRequest event in the agent runner
            return Ok(ToolResult::new(
                "Awaiting plan approval",
                format!(
                    "PENDING: Plan approval requested\n\n\
                     Plan file: {}\n\
                     Content length: {} chars\n\n\
                     User will review and approve/reject the plan.",
                    plan_file.display(),
                    plan_content.len()
                ),
            )
            .with_metadata("status", json!("pending"))
            .with_metadata("plan_file", json!(plan_file.to_string_lossy()))
            .with_metadata("plan_length", json!(plan_content.len())));
        }

        // Phase 2: User decision received
        let approved = params.approved.unwrap();

        if approved {
            tracing::info!(
                session_id = %ctx.session_id,
                "tool exit_plan_mode: plan approved, exiting plan mode"
            );

            let output = format!(
                "✅ Plan approved! Exiting plan mode.\n\n\
                 All tools are now available for implementation.\n\
                 You can now proceed with the implementation according to the plan.\n\n\
                 Plan file: {}",
                plan_file.display()
            );

            Ok(ToolResult::new("Plan approved", output)
                .with_metadata("status", json!("approved"))
                .with_metadata("plan_file", json!(plan_file.to_string_lossy())))
        } else {
            tracing::info!(
                session_id = %ctx.session_id,
                "tool exit_plan_mode: plan rejected, staying in plan mode"
            );

            let output = format!(
                "❌ Plan rejected by user.\n\n\
                 You are still in plan mode. Please revise your plan based on user feedback.\n\
                 Plan file: {}",
                plan_file.display()
            );

            Ok(ToolResult::new("Plan rejected", output)
                .with_metadata("status", json!("rejected"))
                .with_metadata("plan_file", json!(plan_file.to_string_lossy())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    async fn create_test_context_with_plan() -> (ToolContext, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();

        // Create .claude directory and plan file
        let claude_dir = temp_dir.path().join(".claude");
        tokio::fs::create_dir_all(&claude_dir).await.unwrap();

        let plan_file = claude_dir.join("plan-test-session.md");
        tokio::fs::write(&plan_file, "# Test Plan\nThis is a test plan.")
            .await
            .unwrap();

        let ctx = ToolContext {
            session_id: "test-session".to_string(),
            message_id: "test-message".to_string(),
            agent: "test-agent".to_string(),
            working_dir: temp_dir.path().to_path_buf(),
            shell_manager: Arc::new(crate::process::BackgroundShellManager::new()),
        };

        (ctx, temp_dir)
    }

    #[tokio::test]
    async fn test_exit_plan_mode_no_file() {
        let tool = ExitPlanModeTool;
        let temp_dir = tempdir().unwrap();

        let ctx = ToolContext {
            session_id: "test-session".to_string(),
            message_id: "test-message".to_string(),
            agent: "test-agent".to_string(),
            working_dir: temp_dir.path().to_path_buf(),
            shell_manager: Arc::new(crate::process::BackgroundShellManager::new()),
        };

        let params = json!({});
        let result = tool.execute(params, &ctx).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn test_phase_1_awaiting_approval() {
        let tool = ExitPlanModeTool;
        let (ctx, _temp_dir) = create_test_context_with_plan().await;

        let params = json!({});
        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "Awaiting plan approval");
        assert!(result.output.contains("PENDING"));
        assert_eq!(result.metadata.get("status").unwrap(), &json!("pending"));
    }

    #[tokio::test]
    async fn test_phase_2_approved() {
        let tool = ExitPlanModeTool;
        let (ctx, _temp_dir) = create_test_context_with_plan().await;

        let params = json!({ "approved": true });
        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "Plan approved");
        assert!(result.output.contains("approved"));
        assert_eq!(result.metadata.get("status").unwrap(), &json!("approved"));
    }

    #[tokio::test]
    async fn test_phase_2_rejected() {
        let tool = ExitPlanModeTool;
        let (ctx, _temp_dir) = create_test_context_with_plan().await;

        let params = json!({ "approved": false });
        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "Plan rejected");
        assert!(result.output.contains("rejected"));
        assert_eq!(result.metadata.get("status").unwrap(), &json!("rejected"));
    }

    #[tokio::test]
    async fn test_reads_plan_content() {
        let tool = ExitPlanModeTool;
        let (ctx, temp_dir) = create_test_context_with_plan().await;

        // Verify plan file exists and has content
        let plan_file = temp_dir.path().join(".claude/plan-test-session.md");
        assert!(plan_file.exists());

        let params = json!({});
        let result = tool.execute(params, &ctx).await.unwrap();

        // Should have read the plan content
        assert!(result
            .metadata
            .get("plan_length")
            .unwrap()
            .as_u64()
            .unwrap()
            > 0);
    }
}
