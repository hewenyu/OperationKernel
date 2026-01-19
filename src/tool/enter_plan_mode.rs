use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;

/// EnterPlanMode tool - puts the agent into a restricted planning mode
///
/// In plan mode:
/// - Agent can only use read-only tools (Read, Grep, Glob, Bash with restrictions)
/// - Focus is on exploration and design, not implementation
/// - Agent writes a plan to a designated file
/// - User must approve the plan via ExitPlanMode before implementation begins
pub struct EnterPlanModeTool;

/// Input parameters for EnterPlanMode (currently no parameters needed)
#[derive(Debug, Deserialize)]
struct EnterPlanModeParams {}

#[async_trait::async_trait]
impl Tool for EnterPlanModeTool {
    fn id(&self) -> &str {
        "enter_plan_mode"
    }

    fn description(&self) -> &str {
        "Enter plan mode to design implementation approach before writing code. \
         In plan mode, only exploration tools (Read, Grep, Glob) are available. \
         Use this when the task requires planning the implementation steps of a task \
         that requires writing code. For research tasks, do NOT use this tool."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let _params: EnterPlanModeParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        tracing::info!(
            session_id = %ctx.session_id,
            "tool enter_plan_mode: entering plan mode"
        );

        // Determine plan file path
        let plan_file = ctx
            .working_dir
            .join(".claude")
            .join(format!("plan-{}.md", ctx.session_id));

        // Ensure .claude directory exists
        if let Some(parent) = plan_file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::Other(e.into()))?;
        }

        // Create initial plan file with template
        let template = format!(
            r#"# Implementation Plan

## Task Overview
[Describe the task to be implemented]

## Current Understanding
[What you've learned from exploring the codebase]

## Implementation Approach
[High-level strategy]

## Step-by-Step Plan
1. [First step]
2. [Second step]
3. ...

## Files to Create/Modify
- `path/to/file.rs` - [What changes]
- ...

## Risks and Considerations
- [Potential issues or decisions needed]

## Questions for User
- [Any clarifications needed before proceeding]

---
*Plan file: {}*
*Session: {}*
"#,
            plan_file.display(),
            ctx.session_id
        );

        tokio::fs::write(&plan_file, template)
            .await
            .map_err(|e| ToolError::Other(e.into()))?;

        let output = format!(
            "✅ Entered plan mode\n\n\
             In plan mode, you can:\n\
             - Explore the codebase (Read, Grep, Glob)\n\
             - Design the implementation approach\n\
             - Write your plan to: {}\n\n\
             When ready, use the exit_plan_mode tool to request user approval.\n\n\
             ⚠️  Implementation tools (Write, Edit) are disabled until plan is approved.",
            plan_file.display()
        );

        tracing::debug!(
            session_id = %ctx.session_id,
            plan_file = %plan_file.display(),
            "tool enter_plan_mode done"
        );

        Ok(ToolResult::new("Entered plan mode", output)
            .with_metadata("plan_file", json!(plan_file.to_string_lossy()))
            .with_metadata("status", json!("active")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_context() -> (ToolContext, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
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
    async fn test_enter_plan_mode() {
        let tool = EnterPlanModeTool;
        let (ctx, temp_dir) = create_test_context();

        let params = json!({});
        let result = tool.execute(params, &ctx).await.unwrap();

        assert_eq!(result.title, "Entered plan mode");
        assert!(result.output.contains("plan mode"));

        // Verify plan file was created
        let plan_file = temp_dir
            .path()
            .join(".claude")
            .join("plan-test-session.md");
        assert!(plan_file.exists());

        // Verify file contains template
        let content = tokio::fs::read_to_string(&plan_file).await.unwrap();
        assert!(content.contains("# Implementation Plan"));
        assert!(content.contains("## Task Overview"));
    }

    #[tokio::test]
    async fn test_creates_claude_directory() {
        let tool = EnterPlanModeTool;
        let (ctx, temp_dir) = create_test_context();

        let claude_dir = temp_dir.path().join(".claude");
        assert!(!claude_dir.exists());

        let params = json!({});
        tool.execute(params, &ctx).await.unwrap();

        assert!(claude_dir.exists());
        assert!(claude_dir.is_dir());
    }
}
