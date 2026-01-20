use crate::llm::anthropic::AnthropicClient;
use crate::subagent::config::{SubagentConfig, SubagentType};
use crate::subagent::runner::SubagentRunner;
use crate::tool::base::{Tool, ToolContext, ToolError, ToolResult};
use crate::tool::ToolRegistry;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Task tool - launches specialized subagents for complex multi-step tasks
///
/// Available subagent types:
/// - **Explore**: Fast codebase exploration (read, grep, glob, bash)
/// - **Plan**: Implementation planning architect (read, grep, glob, bash)
///
/// Each subagent runs independently with:
/// - Filtered tool access (only tools appropriate for the task)
/// - Custom system prompt (specialized instructions)
/// - Turn limit (max 10 turns in MVP)
/// - Independent conversation history
pub struct TaskTool {
    llm_client: Arc<AnthropicClient>,
}

impl TaskTool {
    /// Create a new TaskTool with the given LLM client
    pub fn new(llm_client: Arc<AnthropicClient>) -> Self {
        Self { llm_client }
    }

    /// Create a filtered tool registry containing only tools allowed for the subagent
    ///
    /// This enforces security boundaries by preventing subagents from accessing
    /// tools outside their authorization scope.
    fn create_filtered_registry(&self, config: &SubagentConfig) -> Arc<ToolRegistry> {
        let full_registry = ToolRegistry::new();

        // If wildcard "*" is present, allow all tools
        if config.available_tools.contains(&"*".to_string()) {
            tracing::debug!(
                subagent_type = %config.name,
                "subagent has access to all tools"
            );
            return Arc::new(full_registry);
        }

        // Otherwise, create a filtered registry with only allowed tools
        let mut filtered_tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        for tool_name in &config.available_tools {
            if let Some(tool) = full_registry.get(tool_name) {
                filtered_tools.insert(tool_name.clone(), tool.clone());
            } else {
                tracing::warn!(
                    subagent_type = %config.name,
                    tool_name = %tool_name,
                    "configured tool not found in registry"
                );
            }
        }

        tracing::debug!(
            subagent_type = %config.name,
            tool_count = filtered_tools.len(),
            tools = ?filtered_tools.keys().collect::<Vec<_>>(),
            "created filtered tool registry for subagent"
        );

        Arc::new(ToolRegistry::from_map(filtered_tools))
    }
}

#[derive(Debug, Deserialize)]
struct TaskParams {
    /// Short 3-5 word description of the task
    description: String,
    /// Detailed task prompt for the subagent
    prompt: String,
    /// Type of specialized subagent to use
    subagent_type: String,
    /// Optional model to use (sonnet, opus, haiku)
    #[serde(default = "default_model")]
    model: String,
}

fn default_model() -> String {
    "sonnet".to_string()
}

#[async_trait]
impl Tool for TaskTool {
    fn id(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Launch a specialized subagent to handle complex, multi-step tasks autonomously.\n\n\
         Available subagent types:\n\
         - **Explore**: Fast codebase exploration specialist\n\
         - **Plan**: Implementation planning architect\n\n\
         Each subagent has filtered tool access and specialized prompts for its role.\n\n\
         Usage:\n\
         - Use Explore to find files, understand code structure, or answer architecture questions\n\
         - Use Plan to design implementation approaches for new features\n\
         - Subagents run independently and return their results when complete\n\
         - Max 10 conversation turns per subagent (MVP limitation)"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The detailed task for the subagent to perform"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "The type of specialized subagent to use",
                    "enum": ["Explore", "Plan"]
                },
                "model": {
                    "type": "string",
                    "enum": ["sonnet", "opus", "haiku"],
                    "description": "Optional model to use (defaults to sonnet). \
                                    Use haiku for simple tasks to reduce cost."
                }
            },
            "required": ["description", "prompt", "subagent_type"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: TaskParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        // Parse and validate subagent type
        let subagent_type: SubagentType = params
            .subagent_type
            .parse()
            .map_err(|e: anyhow::Error| ToolError::InvalidParams(e.to_string()))?;

        // MVP: Only support Explore and Plan
        match subagent_type {
            SubagentType::Explore | SubagentType::Plan => {}
            _ => {
                return Err(ToolError::InvalidParams(format!(
                    "Subagent type '{}' not supported in MVP. \
                     Supported types: Explore, Plan",
                    params.subagent_type
                )))
            }
        }

        // Generate unique subagent ID
        let agent_id = uuid::Uuid::new_v4().to_string();

        tracing::info!(
            parent_session = %ctx.session_id,
            agent_id = %agent_id,
            subagent_type = %params.subagent_type,
            description = %params.description,
            "launching subagent"
        );

        // Get subagent configuration
        let config = SubagentConfig::for_type(&subagent_type);

        // Create filtered tool registry (security boundary)
        let filtered_registry = self.create_filtered_registry(&config);

        // Create and run subagent
        let mut subagent_runner = SubagentRunner::new(
            agent_id.clone(),
            config,
            filtered_registry,
            ctx.working_dir.clone(),
            (*self.llm_client).clone(),
        );

        // Execute the task (blocks until complete or max turns)
        let result = subagent_runner
            .run_task(params.prompt)
            .await
            .map_err(|e| {
                tracing::error!(
                    agent_id = %agent_id,
                    error = %e,
                    "subagent task failed"
                );
                ToolError::Other(anyhow::anyhow!("Subagent execution failed: {}", e))
            })?;

        tracing::info!(
            agent_id = %agent_id,
            turns = result.turns,
            output_len = result.output.len(),
            "subagent task completed successfully"
        );

        // Return formatted result
        Ok(ToolResult::new(
            format!("Subagent task: {}", params.description),
            result.output,
        )
        .with_metadata("agent_id", json!(agent_id))
        .with_metadata("subagent_type", json!(params.subagent_type))
        .with_metadata("turns", json!(result.turns))
        .with_metadata("conversation_length", json!(result.conversation.len())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::station::{Provider, Station};
    use crate::process::BackgroundShellManager;
    use std::path::PathBuf;

    fn create_test_llm_client() -> Arc<AnthropicClient> {
        let station = Station {
            id: "test".to_string(),
            name: "test".to_string(),
            provider: Provider::Anthropic,
            model: "claude-3-sonnet-20240229".to_string(),
            api_key: "test-key".to_string(),
            api_base: None,
            max_tokens: Some(1024),
            temperature: Some(1.0),
        };
        Arc::new(AnthropicClient::new(station))
    }

    fn create_test_context() -> ToolContext {
        ToolContext {
            session_id: "test-session".to_string(),
            message_id: "test-msg".to_string(),
            agent: "test-agent".to_string(),
            working_dir: PathBuf::from("/tmp"),
            shell_manager: Arc::new(BackgroundShellManager::new()),
        }
    }

    #[test]
    fn test_task_tool_creation() {
        let llm_client = create_test_llm_client();
        let tool = TaskTool::new(llm_client);
        assert_eq!(tool.id(), "task");
    }

    #[test]
    fn test_filtered_registry_creation() {
        let llm_client = create_test_llm_client();
        let tool = TaskTool::new(llm_client);

        let config = SubagentConfig::for_type(&SubagentType::Explore);
        let filtered = tool.create_filtered_registry(&config);

        // Explore should only have read-only tools
        assert!(filtered.get("read").is_some());
        assert!(filtered.get("grep").is_some());
        assert!(filtered.get("glob").is_some());
        assert!(filtered.get("bash").is_some());

        // Should NOT have write tools
        assert!(filtered.get("write").is_none());
        assert!(filtered.get("edit").is_none());
    }

    #[test]
    fn test_filtered_registry_general_purpose() {
        let llm_client = create_test_llm_client();
        let tool = TaskTool::new(llm_client);

        let config = SubagentConfig::for_type(&SubagentType::GeneralPurpose);
        let filtered = tool.create_filtered_registry(&config);

        // GeneralPurpose should have all tools (uses "*")
        assert!(filtered.get("read").is_some());
        assert!(filtered.get("write").is_some());
        assert!(filtered.get("edit").is_some());
    }

    #[tokio::test]
    async fn test_invalid_subagent_type() {
        let llm_client = create_test_llm_client();
        let tool = TaskTool::new(llm_client);
        let ctx = create_test_context();

        let params = json!({
            "description": "Test task",
            "prompt": "Do something",
            "subagent_type": "InvalidType"
        });

        let result = tool.execute(params, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bash_subagent_not_supported_in_mvp() {
        let llm_client = create_test_llm_client();
        let tool = TaskTool::new(llm_client);
        let ctx = create_test_context();

        let params = json!({
            "description": "Test bash task",
            "prompt": "Run some commands",
            "subagent_type": "Bash"
        });

        let result = tool.execute(params, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not supported in MVP"));
    }
}
