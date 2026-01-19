use crate::event::{Event, EventResult};
use crate::llm::anthropic::AnthropicClient;
use crate::llm::types::{Message, StreamChunk, ToolUse};
use crate::tool::base::ToolContext;
use crate::tool::ToolRegistry;
use crate::tui::{ChatMessage, InputWidget, MessageList};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Main application state
pub struct App {
    /// LLM client
    llm_client: AnthropicClient,
    /// Message list component with scrolling support
    message_list: MessageList,
    /// Current message ID counter
    current_message_id: usize,
    /// Conversation history (for LLM)
    conversation: Vec<Message>,
    /// Input widget for user text
    pub input: InputWidget,
    /// Whether the application should quit
    should_quit: bool,
    /// Whether we're currently waiting for LLM response
    is_loading: bool,
    /// Channel receiver for streaming responses
    stream_receiver: Option<mpsc::UnboundedReceiver<StreamChunk>>,
    /// Tool registry for executing tools
    tool_registry: Arc<ToolRegistry>,
    /// Pending tool calls that need execution
    pending_tool_calls: Vec<ToolUse>,
}

impl App {
    /// Create a new application instance
    pub fn new(llm_client: AnthropicClient) -> Self {
        let mut message_list = MessageList::new();
        let mut current_id = 0;

        // Add welcome messages
        message_list.add_message(ChatMessage::system(
            current_id,
            "Welcome to OperationKernel (OK Agent)!".to_string(),
        ));
        current_id += 1;

        message_list.add_message(ChatMessage::system(
            current_id,
            "Connected to Claude API - Ready to chat!".to_string(),
        ));
        current_id += 1;

        message_list.add_message(ChatMessage::system(
            current_id,
            "Controls: Enter=send | Shift+Enter=newline | ↑↓=scroll | End=bottom | Ctrl+C=quit".to_string(),
        ));
        current_id += 1;

        Self {
            llm_client,
            message_list,
            current_message_id: current_id,
            conversation: Vec::new(),
            input: InputWidget::new(),
            should_quit: false,
            is_loading: false,
            stream_receiver: None,
            tool_registry: Arc::new(ToolRegistry::new()),
            pending_tool_calls: Vec::new(),
        }
    }

    /// Check if the application should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Check for streaming messages
    pub fn poll_stream(&mut self) -> Option<StreamChunk> {
        if let Some(receiver) = &mut self.stream_receiver {
            receiver.try_recv().ok()
        } else {
            None
        }
    }

    /// Handle a stream chunk
    pub fn handle_stream_chunk(&mut self, chunk: StreamChunk) {
        match chunk {
            StreamChunk::Text(text) => {
                // Append to the current streaming message
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&text);
                }
            }
            StreamChunk::ToolUse(tool_use) => {
                // Display tool call in TUI
                let tool_msg = format!("🔧 Calling tool: {} ({})", tool_use.name, tool_use.id);
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&format!("\n\n{}", tool_msg));
                }

                // Store tool use for execution
                self.pending_tool_calls.push(tool_use);
            }
            StreamChunk::Done => {
                // Mark message as complete and add to conversation
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    let content = msg.content.clone();
                    msg.complete();

                    if !content.is_empty() {
                        self.conversation.push(Message::assistant(content));
                    }
                }

                // Execute pending tool calls
                if !self.pending_tool_calls.is_empty() {
                    self.execute_pending_tools();
                } else {
                    self.is_loading = false;
                    self.stream_receiver = None;
                }
            }
            StreamChunk::Error(err) => {
                // Add error message
                self.message_list.add_message(ChatMessage::error(
                    self.current_message_id,
                    err.clone(),
                ));
                self.current_message_id += 1;

                self.is_loading = false;
                self.stream_receiver = None;
                self.pending_tool_calls.clear();
            }
        }
    }

    /// Handle an event
    pub fn handle_event(&mut self, event: Event) -> EventResult<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Quit => {
                self.should_quit = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Handle mouse events
    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> EventResult<()> {
        use crossterm::event::MouseEventKind;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up(3);
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input
    fn handle_key(&mut self, key: KeyEvent) -> EventResult<()> {
        // Handle Ctrl+C to quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
        }

        // Handle scroll keys (Up/Down/PageUp/PageDown/Home/End)
        match key.code {
            KeyCode::Up => {
                self.scroll_up(1);
                return Ok(());
            }
            KeyCode::Down => {
                self.scroll_down(1);
                return Ok(());
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
                return Ok(());
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
                return Ok(());
            }
            KeyCode::Home => {
                self.scroll_to_top();
                return Ok(());
            }
            KeyCode::End => {
                self.scroll_to_bottom();
                return Ok(());
            }
            _ => {}
        }

        // Handle Enter to submit message (Shift+Enter for new line)
        if key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::SHIFT) {
            if !self.is_loading {
                self.submit_message();
            }
            return Ok(());
        }

        // Forward other keys to the input widget (including Shift+Enter)
        self.input.handle_key(key);
        Ok(())
    }

    /// Scroll up by n lines
    fn scroll_up(&mut self, lines: usize) {
        self.message_list.scroll_up(lines as u16);
    }

    /// Scroll down by n lines
    fn scroll_down(&mut self, lines: usize) {
        self.message_list.scroll_down(lines as u16);
    }

    /// Scroll to the top
    fn scroll_to_top(&mut self) {
        self.message_list.scroll_up(u16::MAX);
    }

    /// Scroll to the bottom (re-enable auto-scroll)
    fn scroll_to_bottom(&mut self) {
        self.message_list.enable_auto_scroll();
    }

    /// Execute all pending tool calls
    fn execute_pending_tools(&mut self) {
        let tool_calls = std::mem::take(&mut self.pending_tool_calls);
        let registry = self.tool_registry.clone();
        let working_dir = std::env::current_dir().unwrap_or_default();

        // Create a message for tool execution status
        let tool_status_msg = ChatMessage::system(
            self.current_message_id,
            format!("⚙️  Executing {} tool(s)...", tool_calls.len()),
        );
        self.message_list.add_message(tool_status_msg);
        self.current_message_id += 1;

        // Clone conversation for the async task
        let mut conversation = self.conversation.clone();
        let llm_client = self.llm_client.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_receiver = Some(rx);

        // Create streaming assistant message for the response
        let assistant_msg = ChatMessage::assistant_streaming(self.current_message_id);
        self.message_list.add_message(assistant_msg);
        self.current_message_id += 1;

        // Spawn async task to execute tools and continue conversation
        tokio::spawn(async move {
            use crate::llm::types::Message;

            // Execute each tool sequentially
            for tool_use in tool_calls {
                // 1. Get the tool from registry
                let tool = match registry.get(&tool_use.name) {
                    Some(t) => t,
                    None => {
                        // Tool not found - add error result
                        let error_msg = format!("Tool '{}' not found", tool_use.name);
                        conversation.push(Message::user_with_tool_result(
                            tool_use.id,
                            error_msg,
                        ));
                        continue;
                    }
                };

                // 2. Create tool execution context
                let ctx = ToolContext::new(
                    "session_1",
                    "msg_1",
                    "default",
                    working_dir.clone(),
                );

                // 3. Execute the tool
                let result = tool.execute(tool_use.input, &ctx).await;

                // 4. Format result and add to conversation
                let result_content = match result {
                    Ok(tool_result) => {
                        // Successful execution - format output
                        format!(
                            "Tool: {}\nOutput:\n{}",
                            tool_result.title,
                            tool_result.output
                        )
                    }
                    Err(e) => {
                        // Tool execution failed - include error
                        format!("Tool execution failed: {}", e)
                    }
                };

                conversation.push(Message::user_with_tool_result(
                    tool_use.id,
                    result_content,
                ));
            }

            // 5. After all tools execute, continue the conversation with Claude
            let tool_definitions = Some(registry.list_tool_definitions());
            match llm_client.stream_chat(conversation, tool_definitions).await {
                Ok(mut stream) => {
                    use futures::StreamExt;
                    while let Some(chunk) = stream.next().await {
                        if tx.send(chunk).is_err() {
                            break; // Receiver dropped
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamChunk::Error(e.to_string()));
                }
            }
        });
    }

    /// Submit the current message and start LLM streaming
    fn submit_message(&mut self) {
        let text = self.input.take_text();
        if text.trim().is_empty() {
            return;
        }

        // Add user message
        let user_msg = ChatMessage::user(self.current_message_id, text.clone());
        self.message_list.add_message(user_msg);
        self.current_message_id += 1;

        self.conversation.push(Message::user(text));

        // Start loading and create streaming assistant message
        self.is_loading = true;
        let assistant_msg = ChatMessage::assistant_streaming(self.current_message_id);
        self.message_list.add_message(assistant_msg);
        self.current_message_id += 1;

        // Create a channel for streaming
        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_receiver = Some(rx);

        // Clone what we need for the async task
        let llm_client = self.llm_client.clone();
        let conversation = self.conversation.clone();
        let tool_definitions = Some(self.tool_registry.list_tool_definitions());

        // Spawn async task to call LLM
        tokio::spawn(async move {
            match llm_client.stream_chat(conversation, tool_definitions).await {
                Ok(mut stream) => {
                    use futures::StreamExt;
                    while let Some(chunk) = stream.next().await {
                        if tx.send(chunk).is_err() {
                            break; // Receiver dropped
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamChunk::Error(e.to_string()));
                }
            }
        });
    }

    /// Render the application UI
    pub fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),     // Chat history
                Constraint::Length(3),  // Status bar
                Constraint::Length(5),  // Input area
            ])
            .split(frame.area());

        self.render_chat(frame, chunks[0]);
        self.render_status(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
    }

    /// Render chat history
    fn render_chat(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::BorderType;
        
        // Create a block for the chat area
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)  // Unified rounded borders
            .title(Span::styled(
                " 💬 Chat History ",
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD)
            ))
            .border_style(Style::default().fg(Color::DarkGray));  // Subtle border color

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Render the message list inside the block
        self.message_list.render(frame, inner);
    }

    /// Render status bar
    fn render_status(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::BorderType;
        
        let status_text = vec![Line::from(vec![
            Span::styled(
                if self.is_loading { "⚡ " } else { "✓ " },
                Style::default().fg(if self.is_loading { Color::Yellow } else { Color::Green })
            ),
            Span::styled("Status: ", Style::default().fg(Color::LightYellow)),
            Span::raw(if self.is_loading {
                "Generating..."
            } else {
                "Ready"
            }),
            Span::raw(" │ "),  // Visual separator
            Span::styled("Messages: ", Style::default().fg(Color::LightCyan)),
            Span::raw(self.message_list.len().to_string()),
        ])];

        let status = Paragraph::new(status_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)  // Unified rounded borders
                .title(Span::styled(
                    " 📊 Status ",
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD)
                ))
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        frame.render_widget(status, area);
    }

    /// Render input area
    fn render_input(&mut self, frame: &mut Frame, area: Rect) {
        self.input.render(frame, area);
    }
}
