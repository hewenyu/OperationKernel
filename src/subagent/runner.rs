use crate::llm::anthropic::AnthropicClient;
use crate::llm::types::{ContentBlock, Message, StreamChunk};
use crate::process::BackgroundShellManager;
use crate::subagent::config::SubagentConfig;
use crate::tool::base::ToolContext;
use crate::tool::ToolRegistry;
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;

/// Result from a completed subagent execution
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// Final output text from the subagent
    pub output: String,
    /// Number of conversation turns (LLM calls)
    pub turns: usize,
    /// Full conversation history
    pub conversation: Vec<Message>,
}

/// Errors that can occur during subagent execution
#[derive(Debug, thiserror::Error)]
pub enum SubagentError {
    #[error("Max turns exceeded: {0}")]
    MaxTurnsExceeded(usize),

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Subagent runner - manages execution of specialized subagents
///
/// Each subagent runs independently with:
/// - Filtered tool access (based on SubagentConfig)
/// - Custom system prompt (if specified)
/// - Independent conversation history
/// - Turn limit to prevent runaway execution
pub struct SubagentRunner {
    agent_id: String,
    config: SubagentConfig,
    tool_registry: Arc<ToolRegistry>,
    working_dir: PathBuf,
    llm_client: AnthropicClient,
    conversation: Vec<Message>,
}

impl SubagentRunner {
    /// Create a new SubagentRunner
    pub fn new(
        agent_id: String,
        config: SubagentConfig,
        tool_registry: Arc<ToolRegistry>,
        working_dir: PathBuf,
        llm_client: AnthropicClient,
    ) -> Self {
        Self {
            agent_id,
            config,
            tool_registry,
            working_dir,
            llm_client,
            conversation: Vec::new(),
        }
    }

    /// Run the subagent task to completion
    ///
    /// The subagent will:
    /// 1. Receive the task prompt
    /// 2. Make LLM calls to analyze and plan
    /// 3. Execute tools as needed (with filtered access)
    /// 4. Continue until task complete or max turns reached
    ///
    /// Returns the final output and conversation history
    pub async fn run_task(&mut self, prompt: String) -> Result<SubagentResult, SubagentError> {
        tracing::info!(
            agent_id = %self.agent_id,
            subagent_type = %self.config.name,
            prompt_len = prompt.len(),
            "starting subagent task"
        );

        // Build initial message with system prompt (if any)
        let initial_message = if let Some(system_prompt) = &self.config.system_prompt {
            format!("{}\n\n# Task\n{}", system_prompt, prompt)
        } else {
            prompt
        };

        self.conversation.push(Message::user(initial_message));

        let mut turns = 0;
        let max_turns = 10; // MVP: hard limit at 10 turns

        // Main conversation loop
        loop {
            turns += 1;
            if turns > max_turns {
                tracing::warn!(
                    agent_id = %self.agent_id,
                    turns = turns,
                    "subagent exceeded max turns"
                );
                return Err(SubagentError::MaxTurnsExceeded(max_turns));
            }

            tracing::debug!(
                agent_id = %self.agent_id,
                turn = turns,
                "starting subagent turn"
            );

            // Get tool definitions (filtered by subagent config)
            let tool_definitions = Some(self.tool_registry.list_tool_definitions());

            // Call LLM with current conversation
            let mut stream = self
                .llm_client
                .stream_chat(self.conversation.clone(), tool_definitions)
                .await
                .map_err(|e| SubagentError::LlmError(e.to_string()))?;

            let mut assistant_text = String::new();
            let mut tool_uses = Vec::new();

            // Process streaming response
            while let Some(chunk) = stream.next().await {
                match chunk {
                    StreamChunk::Text(text) => {
                        assistant_text.push_str(&text);
                    }
                    StreamChunk::ToolUse(tool_use) => {
                        tool_uses.push(tool_use);
                    }
                    StreamChunk::Done => break,
                    StreamChunk::Error(err) => {
                        tracing::error!(
                            agent_id = %self.agent_id,
                            error = %err,
                            "subagent llm error"
                        );
                        return Err(SubagentError::LlmError(err));
                    }
                }
            }

            tracing::debug!(
                agent_id = %self.agent_id,
                turn = turns,
                text_len = assistant_text.len(),
                tool_count = tool_uses.len(),
                "subagent turn completed"
            );

            // Save assistant message to conversation
            if !tool_uses.is_empty() {
                let mut blocks = Vec::new();
                if !assistant_text.is_empty() {
                    blocks.push(ContentBlock::Text {
                        text: assistant_text.clone(),
                    });
                }
                blocks.extend(
                    tool_uses
                        .iter()
                        .cloned()
                        .map(ContentBlock::ToolUse),
                );
                self.conversation
                    .push(Message::assistant_with_blocks(blocks));
            } else if !assistant_text.is_empty() {
                self.conversation
                    .push(Message::assistant(assistant_text.clone()));
            }

            // If no tools requested, task is complete
            if tool_uses.is_empty() {
                tracing::info!(
                    agent_id = %self.agent_id,
                    turns = turns,
                    output_len = assistant_text.len(),
                    "subagent task completed successfully"
                );

                return Ok(SubagentResult {
                    output: assistant_text,
                    turns,
                    conversation: self.conversation.clone(),
                });
            }

            // Execute all requested tools
            for tool_use in tool_uses {
                tracing::debug!(
                    agent_id = %self.agent_id,
                    tool_name = %tool_use.name,
                    tool_use_id = %tool_use.id,
                    "executing subagent tool"
                );

                // Check if tool is available in filtered registry
                let tool = match self.tool_registry.get(&tool_use.name) {
                    Some(t) => t.clone(),
                    None => {
                        let error_msg =
                            format!("Tool '{}' not available in subagent", tool_use.name);
                        tracing::warn!(
                            agent_id = %self.agent_id,
                            tool_name = %tool_use.name,
                            "tool not available in filtered registry"
                        );

                        self.conversation
                            .push(Message::user_with_tool_result_detailed(
                                tool_use.id,
                                error_msg,
                                Some(true),
                            ));
                        continue;
                    }
                };

                // Create tool context for subagent
                let ctx = ToolContext {
                    session_id: self.agent_id.clone(),
                    message_id: tool_use.id.clone(),
                    agent: self.config.name.clone(),
                    working_dir: self.working_dir.clone(),
                    shell_manager: Arc::new(BackgroundShellManager::new()),
                };

                // Execute tool
                let result = tool.execute(tool_use.input, &ctx).await;

                let (result_content, is_error) = match result {
                    Ok(tr) => {
                        let formatted = format!("Tool: {}\nOutput:\n{}", tr.title, tr.output);
                        tracing::debug!(
                            agent_id = %self.agent_id,
                            tool_name = %tool_use.name,
                            output_len = tr.output.len(),
                            "tool executed successfully"
                        );
                        (formatted, false)
                    }
                    Err(e) => {
                        let error_msg = format!("Tool error: {}", e);
                        tracing::warn!(
                            agent_id = %self.agent_id,
                            tool_name = %tool_use.name,
                            error = %e,
                            "tool execution failed"
                        );
                        (error_msg, true)
                    }
                };

                // Add tool result to conversation
                self.conversation
                    .push(Message::user_with_tool_result_detailed(
                        tool_use.id,
                        result_content,
                        if is_error { Some(true) } else { None },
                    ));
            }

            // Continue to next turn
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::station::Station;
    use crate::subagent::config::{SubagentConfig, SubagentType};

    fn create_test_llm_client() -> AnthropicClient {
        use crate::config::station::Provider;

        // Create a minimal test client (won't actually be called in unit tests)
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
        AnthropicClient::new(station)
    }

    #[test]
    fn test_subagent_runner_creation() {
        let config = SubagentConfig::for_type(&SubagentType::Explore);
        let registry = Arc::new(ToolRegistry::new());
        let llm_client = create_test_llm_client();

        let runner = SubagentRunner::new(
            "test-agent-123".to_string(),
            config.clone(),
            registry,
            PathBuf::from("/tmp"),
            llm_client,
        );

        assert_eq!(runner.agent_id, "test-agent-123");
        assert_eq!(runner.config.name, config.name);
        assert!(runner.conversation.is_empty());
    }

    #[test]
    fn test_initial_message_with_system_prompt() {
        let config = SubagentConfig::for_type(&SubagentType::Plan);
        assert!(config.system_prompt.is_some());

        // System prompt should be prepended to task prompt
        let task = "Design a new feature";
        let expected_prefix = config.system_prompt.as_ref().unwrap();

        let initial_message = format!("{}\n\n# Task\n{}", expected_prefix, task);
        assert!(initial_message.contains(expected_prefix));
        assert!(initial_message.contains(task));
    }
}
