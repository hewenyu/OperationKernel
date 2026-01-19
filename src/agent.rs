use crate::llm::anthropic::AnthropicClient;
use crate::llm::types::{ContentBlock, Message, StreamChunk, ToolUse};
use crate::process::BackgroundShellManager;
use crate::tool::base::ToolContext;
use crate::tool::ToolRegistry;
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A new assistant message is starting (streaming deltas will follow).
    AssistantStart,
    /// Text delta for the currently streaming assistant message.
    AssistantTextDelta(String),
    /// Tool call requested by the assistant.
    ToolUse(ToolUse),
    /// The current assistant message ended (message_stop).
    AssistantStop,
    /// Tools are about to be executed.
    ToolExecutionStart { count: usize },
    /// A tool finished execution (success or error).
    ToolResult {
        tool_use_id: String,
        tool_name: String,
        content: String,
        is_error: bool,
    },
    /// The whole user turn is complete (no more follow-up tool calls pending).
    TurnComplete,
    /// Fatal error for the current turn.
    Error(String),
}

/// Agent runner: manages conversation state, tool execution, and LLM streaming.
///
/// This is UI-agnostic: it emits `AgentEvent`s that any UI (TUI/CLI/daemon) can consume.
pub struct AgentRunner {
    llm_client: AnthropicClient,
    tool_registry: Arc<ToolRegistry>,
    shell_manager: Arc<BackgroundShellManager>,
    working_dir: PathBuf,
    session_id: String,
    agent_name: String,
    conversation: Arc<Mutex<Vec<Message>>>,
}

impl AgentRunner {
    pub fn new(llm_client: AnthropicClient) -> Self {
        Self {
            llm_client,
            tool_registry: Arc::new(ToolRegistry::new()),
            shell_manager: Arc::new(BackgroundShellManager::new()),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            session_id: "session_1".to_string(),
            agent_name: "ok".to_string(),
            conversation: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    pub fn shell_manager(&self) -> Arc<BackgroundShellManager> {
        self.shell_manager.clone()
    }

    /// Submit a user message and start the agent turn.
    ///
    /// Returns a receiver of `AgentEvent`s for UI consumption.
    pub fn start_turn(&self, user_text: String) -> mpsc::UnboundedReceiver<AgentEvent> {
        let (tx, rx) = mpsc::unbounded_channel();

        let llm_client = self.llm_client.clone();
        let registry = self.tool_registry.clone();
        let shell_manager = self.shell_manager.clone();
        let working_dir = self.working_dir.clone();
        let session_id = self.session_id.clone();
        let agent_name = self.agent_name.clone();
        let conversation = self.conversation.clone();

        tokio::spawn(async move {
            {
                let mut convo = conversation.lock().await;
                convo.push(Message::user(user_text));
            }

            loop {
                if tx.send(AgentEvent::AssistantStart).is_err() {
                    return;
                }

                let conversation_snapshot = { conversation.lock().await.clone() };
                let tool_definitions = Some(registry.list_tool_definitions());

                let mut stream = match llm_client
                    .stream_chat(conversation_snapshot, tool_definitions)
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(AgentEvent::Error(e.to_string()));
                        let _ = tx.send(AgentEvent::TurnComplete);
                        return;
                    }
                };

                let mut assistant_text = String::new();
                let mut assistant_tool_uses: Vec<ToolUse> = Vec::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        StreamChunk::Text(text) => {
                            assistant_text.push_str(&text);
                            if tx.send(AgentEvent::AssistantTextDelta(text)).is_err() {
                                return;
                            }
                        }
                        StreamChunk::ToolUse(tool_use) => {
                            assistant_tool_uses.push(tool_use.clone());
                            if tx.send(AgentEvent::ToolUse(tool_use)).is_err() {
                                return;
                            }
                        }
                        StreamChunk::Done => break,
                        StreamChunk::Error(err) => {
                            let _ = tx.send(AgentEvent::Error(err));
                            let _ = tx.send(AgentEvent::TurnComplete);
                            return;
                        }
                    }
                }

                if tx.send(AgentEvent::AssistantStop).is_err() {
                    return;
                }

                // Persist assistant message to conversation.
                {
                    let mut convo = conversation.lock().await;
                    if !assistant_tool_uses.is_empty() {
                        let mut blocks = Vec::new();
                        if !assistant_text.is_empty() {
                            blocks.push(ContentBlock::Text { text: assistant_text });
                        }
                        blocks.extend(assistant_tool_uses.iter().cloned().map(ContentBlock::ToolUse));
                        convo.push(Message::assistant_with_blocks(blocks));
                    } else if !assistant_text.is_empty() {
                        convo.push(Message::assistant(assistant_text));
                    }
                }

                // No tools => done.
                if assistant_tool_uses.is_empty() {
                    let _ = tx.send(AgentEvent::TurnComplete);
                    return;
                }

                let tool_count = assistant_tool_uses.len();
                if tx
                    .send(AgentEvent::ToolExecutionStart { count: tool_count })
                    .is_err()
                {
                    return;
                }

                // Execute tools sequentially and add tool_result messages.
                for tool_use in assistant_tool_uses {
                    let tool_use_id = tool_use.id.clone();
                    let tool_name = tool_use.name.clone();

                    let tool = match registry.get(&tool_use.name) {
                        Some(t) => t.clone(),
                        None => {
                            let error_msg = format!("Tool '{}' not found", tool_use.name);
                            let _ = tx.send(AgentEvent::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                tool_name: tool_name.clone(),
                                content: error_msg.clone(),
                                is_error: true,
                            });

                            let mut convo = conversation.lock().await;
                            convo.push(Message::user_with_tool_result_detailed(
                                tool_use.id,
                                error_msg,
                                Some(true),
                            ));
                            continue;
                        }
                    };

                    let ctx = ToolContext::new(
                        session_id.clone(),
                        tool_use_id.clone(),
                        agent_name.clone(),
                        working_dir.clone(),
                        shell_manager.clone(),
                    );

                    let result = tool.execute(tool_use.input, &ctx).await;
                    let (result_content, is_error) = match result {
                        Ok(tool_result) => {
                            let formatted = format!(
                                "Tool: {}\nOutput:\n{}",
                                tool_result.title, tool_result.output
                            );
                            (formatted, false)
                        }
                        Err(e) => (format!("Tool execution failed: {}", e), true),
                    };

                    let _ = tx.send(AgentEvent::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        tool_name: tool_name.clone(),
                        content: result_content.clone(),
                        is_error,
                    });

                    let mut convo = conversation.lock().await;
                    convo.push(Message::user_with_tool_result_detailed(
                        tool_use.id,
                        result_content,
                        if is_error { Some(true) } else { None },
                    ));
                }

                // Continue loop: call LLM again with updated conversation.
            }
        });

        rx
    }
}
