use crate::event::{Event, EventResult};
use crate::llm::anthropic::AnthropicClient;
use crate::llm::types::{ContentBlock, Message, StreamChunk, ToolUse};
use crate::process::BackgroundShellManager;
use crate::tool::base::ToolContext;
use crate::tool::ToolRegistry;
use crate::tui::{ChatMessage, ErrorDetails, InputWidget, MessageList};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

/// Spinner frames for loading animation
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone)]
pub(crate) enum AsyncEvent {
    Llm(StreamChunk),
    ToolResult {
        tool_use_id: String,
        tool_name: String,
        content: String,
        is_error: bool,
    },
}

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
    /// Channel receiver for background events (LLM stream + tool results)
    stream_receiver: Option<mpsc::UnboundedReceiver<AsyncEvent>>,
    /// Tool registry for executing tools
    tool_registry: Arc<ToolRegistry>,
    /// Background shell manager for long-running processes
    shell_manager: Arc<BackgroundShellManager>,
    /// Pending tool calls that need execution
    pending_tool_calls: Vec<ToolUse>,
    /// Streaming assistant plain text (excluding tool-call UI annotations)
    streaming_assistant_text: String,
    /// Streaming assistant tool_use blocks (for API follow-up)
    streaming_assistant_tool_uses: Vec<ToolUse>,
    /// Current spinner frame index for loading animation
    spinner_frame: usize,
    /// When streaming started (for showing elapsed time)
    streaming_start_time: Option<Instant>,
    /// Whether the UI needs to be re-rendered
    needs_render: bool,
    /// Last terminal size to detect resizes
    last_terminal_size: (u16, u16),
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
            shell_manager: Arc::new(crate::process::BackgroundShellManager::new()),
            pending_tool_calls: Vec::new(),
            streaming_assistant_text: String::new(),
            streaming_assistant_tool_uses: Vec::new(),
            spinner_frame: 0,
            streaming_start_time: None,
            needs_render: true,  // Start with initial render needed
            last_terminal_size: (0, 0),  // Will be set on first render
        }
    }

    /// Check if the app needs to be rendered
    pub fn needs_render(&self) -> bool {
        self.needs_render
    }

    /// Mark that rendering has been completed
    pub fn mark_rendered(&mut self) {
        self.needs_render = false;
    }

    /// Mark that the UI needs to be re-rendered
    fn mark_dirty(&mut self) {
        self.needs_render = true;
    }

    /// Update terminal size and mark dirty if it changed
    pub fn update_terminal_size(&mut self, width: u16, height: u16) {
        let new_size = (width, height);
        if new_size != self.last_terminal_size {
            self.last_terminal_size = new_size;
            self.mark_dirty();
        }
    }

    /// Update spinner animation frame
    pub fn tick_spinner(&mut self) {
        if self.is_loading {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
            self.mark_dirty();  // Spinner animation needs render
        }
    }

    /// Get current spinner character
    fn get_spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Get elapsed time since streaming started (in seconds)
    fn get_elapsed_time(&self) -> Option<f64> {
        self.streaming_start_time.map(|start| start.elapsed().as_secs_f64())
    }

    /// Check if the application should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Check for streaming messages
    pub(crate) fn poll_stream(&mut self) -> Option<AsyncEvent> {
        if let Some(receiver) = &mut self.stream_receiver {
            receiver.try_recv().ok()
        } else {
            None
        }
    }

    pub(crate) fn handle_async_event(&mut self, event: AsyncEvent) {
        match event {
            AsyncEvent::Llm(chunk) => self.handle_stream_chunk(chunk),
            AsyncEvent::ToolResult {
                tool_use_id,
                tool_name,
                content,
                is_error,
            } => {
                let ui_prefix = if is_error { "❌" } else { "✅" };
                let ui = format!(
                    "\n\n{} Tool result: {} ({})\n{}",
                    ui_prefix, tool_name, tool_use_id, content
                );

                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&ui);
                } else {
                    self.message_list
                        .add_message(ChatMessage::system(self.current_message_id, ui));
                    self.current_message_id += 1;
                }

                self.conversation
                    .push(Message::user_with_tool_result_detailed(
                        tool_use_id,
                        content,
                        if is_error { Some(true) } else { None },
                    ));
                self.mark_dirty();
            }
        }
    }

    /// Handle a stream chunk
    pub fn handle_stream_chunk(&mut self, chunk: StreamChunk) {
        match chunk {
            StreamChunk::Text(text) => {
                tracing::debug!(delta_bytes = text.len(), "stream text delta");
                // Append to the current streaming message
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&text);
                }
                self.streaming_assistant_text.push_str(&text);
                self.mark_dirty();  // New content needs render
            }
            StreamChunk::ToolUse(tool_use) => {
                tracing::debug!(
                    tool_id = %tool_use.id,
                    tool_name = %tool_use.name,
                    "stream tool_use"
                );
                // Display tool call in TUI
                let tool_msg = format!("🔧 Calling tool: {} ({})", tool_use.name, tool_use.id);
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&format!("\n\n{}", tool_msg));
                }

                // Store tool use for execution
                self.streaming_assistant_tool_uses.push(tool_use.clone());
                self.pending_tool_calls.push(tool_use);
                self.mark_dirty();  // Tool use message needs render
            }
            StreamChunk::Done => {
                tracing::debug!(
                    assistant_text_bytes = self.streaming_assistant_text.len(),
                    tool_uses = self.streaming_assistant_tool_uses.len(),
                    pending_tools = self.pending_tool_calls.len(),
                    "stream done"
                );
                // Mark message as complete and add to conversation
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.complete();
                }

                let assistant_text = std::mem::take(&mut self.streaming_assistant_text);
                let assistant_tool_uses = std::mem::take(&mut self.streaming_assistant_tool_uses);

                if !assistant_tool_uses.is_empty() {
                    let mut blocks = Vec::new();
                    if !assistant_text.is_empty() {
                        blocks.push(ContentBlock::Text { text: assistant_text });
                    }
                    blocks.extend(assistant_tool_uses.into_iter().map(ContentBlock::ToolUse));
                    self.conversation.push(Message::assistant_with_blocks(blocks));
                } else if !assistant_text.is_empty() {
                    self.conversation.push(Message::assistant(assistant_text));
                }

                // Execute pending tool calls
                if !self.pending_tool_calls.is_empty() {
                    self.execute_pending_tools();
                } else {
                    self.is_loading = false;
                    self.stream_receiver = None;
                    self.streaming_start_time = None;  // Clear start time
                }
                self.mark_dirty();  // State changed
            }
            StreamChunk::Error(err) => {
                tracing::warn!(error = %crate::logging::redact_secrets(&err), "stream error");
                // Add enhanced error message
                let error_details = ErrorDetails::from_message(err.clone());
                self.message_list.add_message(ChatMessage::error_from_details(
                    self.current_message_id,
                    error_details,
                ));
                self.current_message_id += 1;

                self.is_loading = false;
                self.stream_receiver = None;
                self.streaming_start_time = None;  // Clear start time
                self.pending_tool_calls.clear();
                self.streaming_assistant_text.clear();
                self.streaming_assistant_tool_uses.clear();
                self.mark_dirty();  // Error message needs render
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
                self.mark_dirty();  // Scroll position changed
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
                self.mark_dirty();  // Scroll position changed
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
                self.mark_dirty();
                return Ok(());
            }
            KeyCode::Down => {
                self.scroll_down(1);
                self.mark_dirty();
                return Ok(());
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
                self.mark_dirty();
                return Ok(());
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
                self.mark_dirty();
                return Ok(());
            }
            KeyCode::Home => {
                self.scroll_to_top();
                self.mark_dirty();
                return Ok(());
            }
            KeyCode::End => {
                self.scroll_to_bottom();
                self.mark_dirty();
                return Ok(());
            }
            _ => {}
        }

        // Handle Enter to submit message (Shift+Enter for new line)
        if key.code == KeyCode::Enter && !key.modifiers.contains(KeyModifiers::SHIFT) {
            if !self.is_loading {
                self.submit_message();
                self.mark_dirty();  // New messages added
            }
            return Ok(());
        }

        // Forward other keys to the input widget (including Shift+Enter)
        self.input.handle_key(key);
        self.mark_dirty();  // Input changed
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
        let shell_manager = self.shell_manager.clone();
        let working_dir = std::env::current_dir().unwrap_or_default();

        tracing::debug!(
            working_dir = %working_dir.display(),
            tool_count = tool_calls.len(),
            "executing pending tools"
        );

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
        self.streaming_start_time = Some(Instant::now());  // Restart timing for tool response
        let assistant_msg = ChatMessage::assistant_streaming(self.current_message_id);
        self.message_list.add_message(assistant_msg);
        self.current_message_id += 1;
        self.streaming_assistant_text.clear();
        self.streaming_assistant_tool_uses.clear();

        // Spawn async task to execute tools and continue conversation
        tokio::spawn(async move {
            use crate::llm::types::Message;

            // Execute each tool sequentially
            for tool_use in tool_calls {
                let tool_use_id = tool_use.id.clone();
                let tool_name = tool_use.name.clone();

                // 1. Get the tool from registry
                let tool = match registry.get(&tool_use.name) {
                    Some(t) => t,
                    None => {
                        // Tool not found - add error result
                        let error_msg = format!("Tool '{}' not found", tool_use.name);
                        let _ = tx.send(AsyncEvent::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            tool_name: tool_name.clone(),
                            content: error_msg.clone(),
                            is_error: true,
                        });
                        conversation.push(Message::user_with_tool_result_detailed(
                            tool_use.id,
                            error_msg,
                            Some(true),
                        ));
                        continue;
                    }
                };

                // 2. Create tool execution context
                let ctx = ToolContext::new(
                    "session_1",
                    tool_use_id.clone(),
                    "ok",
                    working_dir.clone(),
                    shell_manager.clone(),
                );

                // 3. Execute the tool
                let result = tool.execute(tool_use.input, &ctx).await;

                // 4. Format result and add to conversation
                let (result_content, is_error) = match result {
                    Ok(tool_result) => {
                        tracing::debug!(
                            tool_name = %tool_name,
                            output_len = tool_result.output.len(),
                            output_preview = &tool_result.output[..tool_result.output.len().min(100)],
                            title = %tool_result.title,
                            "tool execution successful"
                        );

                        // Successful execution - format output
                        let formatted = format!(
                            "Tool: {}\nOutput:\n{}",
                            tool_result.title,
                            tool_result.output
                        );

                        if tool_result.output.is_empty() {
                            tracing::warn!(
                                tool_name = %tool_name,
                                "tool output is empty despite successful execution"
                            );
                        }

                        (formatted, false)
                    }
                    Err(e) => {
                        // Tool execution failed - include error
                        (format!("Tool execution failed: {}", e), true)
                    }
                };

                tracing::debug!(
                    result_content_len = result_content.len(),
                    result_content_preview = &result_content[..result_content.len().min(200)],
                    "sending tool result to UI"
                );

                let _ = tx.send(AsyncEvent::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    tool_name: tool_name.clone(),
                    content: result_content.clone(),
                    is_error,
                });

                conversation.push(Message::user_with_tool_result_detailed(
                    tool_use.id,
                    result_content,
                    if is_error { Some(true) } else { None },
                ));
            }

            // 5. After all tools execute, continue the conversation with Claude
            let tool_definitions = Some(registry.list_tool_definitions());
            match llm_client.stream_chat(conversation, tool_definitions).await {
                Ok(mut stream) => {
                    use futures::StreamExt;
                    while let Some(chunk) = stream.next().await {
                        if tx.send(AsyncEvent::Llm(chunk)).is_err() {
                            break; // Receiver dropped
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AsyncEvent::Llm(StreamChunk::Error(e.to_string())));
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

        tracing::debug!(
            user_bytes = text.len(),
            conversation_len = self.conversation.len(),
            "submit message"
        );

        // Add user message
        let user_msg = ChatMessage::user(self.current_message_id, text.clone());
        self.message_list.add_message(user_msg);
        self.current_message_id += 1;

        self.conversation.push(Message::user(text));

        // Start loading and create streaming assistant message
        self.is_loading = true;
        self.streaming_start_time = Some(Instant::now());  // Start timing
        let assistant_msg = ChatMessage::assistant_streaming(self.current_message_id);
        self.message_list.add_message(assistant_msg);
        self.current_message_id += 1;
        self.streaming_assistant_text.clear();
        self.streaming_assistant_tool_uses.clear();

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
                        if tx.send(AsyncEvent::Llm(chunk)).is_err() {
                            break; // Receiver dropped
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AsyncEvent::Llm(StreamChunk::Error(e.to_string())));
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

        let mut status_spans = vec![
            Span::styled(
                if self.is_loading {
                    format!("{} ", self.get_spinner())
                } else {
                    "✓ ".to_string()
                },
                Style::default().fg(if self.is_loading { Color::Yellow } else { Color::Green })
            ),
            Span::styled("Status: ", Style::default().fg(Color::LightYellow)),
        ];

        // Add status text with elapsed time if loading
        if self.is_loading {
            if let Some(elapsed) = self.get_elapsed_time() {
                status_spans.push(Span::raw(format!("Generating... {:.1}s", elapsed)));
            } else {
                status_spans.push(Span::raw("Generating..."));
            }
        } else {
            status_spans.push(Span::raw("Ready"));
        }

        status_spans.extend_from_slice(&[
            Span::raw(" │ "),  // Visual separator
            Span::styled("Messages: ", Style::default().fg(Color::LightCyan)),
            Span::raw(self.message_list.len().to_string()),
        ]);

        let status_text = vec![Line::from(status_spans)];

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
