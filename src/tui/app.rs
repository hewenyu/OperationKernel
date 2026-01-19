use crate::event::{Event, EventResult};
use crate::agent::{AgentEvent, AgentRunner};
use crate::llm::anthropic::AnthropicClient;
use crate::tui::{ChatMessage, ErrorDetails, InputWidget, MessageList};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::time::Instant;
use tokio::sync::mpsc;

/// Spinner frames for loading animation
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Main application state
pub struct App {
    /// UI-agnostic agent runner (conversation + tool loop)
    agent: AgentRunner,
    /// Message list component with scrolling support
    message_list: MessageList,
    /// Current message ID counter
    current_message_id: usize,
    /// Input widget for user text
    pub input: InputWidget,
    /// Whether the application should quit
    should_quit: bool,
    /// Whether we're currently waiting for LLM response
    is_loading: bool,
    /// Channel receiver for background events (LLM stream + tool results)
    stream_receiver: Option<mpsc::UnboundedReceiver<AgentEvent>>,
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
            agent: AgentRunner::new(llm_client),
            message_list,
            current_message_id: current_id,
            input: InputWidget::new(),
            should_quit: false,
            is_loading: false,
            stream_receiver: None,
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
    #[allow(dead_code)]
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

    /// Check for agent events.
    pub(crate) fn poll_stream(&mut self) -> Option<AgentEvent> {
        if let Some(receiver) = &mut self.stream_receiver {
            receiver.try_recv().ok()
        } else {
            None
        }
    }

    pub(crate) fn handle_async_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::AssistantStart => {
                self.streaming_start_time = Some(Instant::now());
                let assistant_msg = ChatMessage::assistant_streaming(self.current_message_id);
                self.message_list.add_message(assistant_msg);
                self.current_message_id += 1;
                self.mark_dirty();
            }
            AgentEvent::AssistantTextDelta(text) => {
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&text);
                    self.mark_dirty();
                }
            }
            AgentEvent::ToolUse(tool_use) => {
                let tool_msg = format!("🔧 Calling tool: {} ({})", tool_use.name, tool_use.id);
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.append_content(&format!("\n\n{}", tool_msg));
                } else {
                    self.message_list
                        .add_message(ChatMessage::system(self.current_message_id, tool_msg));
                    self.current_message_id += 1;
                }
                self.mark_dirty();
            }
            AgentEvent::AssistantStop => {
                if let Some(msg) = self.message_list.get_current_streaming_mut() {
                    msg.complete();
                    self.mark_dirty();
                }
            }
            AgentEvent::ToolExecutionStart { count } => {
                let tool_status_msg = ChatMessage::system(
                    self.current_message_id,
                    format!("⚙️  Executing {} tool(s)...", count),
                );
                self.message_list.add_message(tool_status_msg);
                self.current_message_id += 1;
                self.mark_dirty();
            }
            AgentEvent::ToolResult {
                tool_use_id,
                tool_name,
                content,
                is_error,
            } => {
                let ui_prefix = if is_error { "❌" } else { "✅" };
                let ui = format!("{} Tool result: {} ({})\n{}", ui_prefix, tool_name, tool_use_id, content);
                self.message_list
                    .add_message(ChatMessage::system(self.current_message_id, ui));
                self.current_message_id += 1;
                self.mark_dirty();
            }
            AgentEvent::TurnComplete => {
                self.is_loading = false;
                self.stream_receiver = None;
                self.streaming_start_time = None;
                self.mark_dirty();
            }
            AgentEvent::Error(err) => {
                let error_details = ErrorDetails::from_message(err);
                self.message_list.add_message(ChatMessage::error_from_details(
                    self.current_message_id,
                    error_details,
                ));
                self.current_message_id += 1;
                self.is_loading = false;
                self.stream_receiver = None;
                self.streaming_start_time = None;
                self.mark_dirty();
            }
            AgentEvent::UserQuestionRequest {
                tool_use_id,
                questions,
            } => {
                // TODO: Implement QuestionWidget UI
                // For now, just display a message
                let msg = format!(
                    "📋 User input requested (tool_use_id: {})\n{} question(s) pending...",
                    tool_use_id,
                    questions.len()
                );
                self.message_list
                    .add_message(ChatMessage::system(self.current_message_id, msg));
                self.current_message_id += 1;
                self.mark_dirty();
            }
            AgentEvent::UserQuestionResponse {
                tool_use_id,
                answers,
            } => {
                // TODO: Process user response
                let msg = format!(
                    "✅ User responses received (tool_use_id: {})\n{} answer(s) provided",
                    tool_use_id,
                    answers.len()
                );
                self.message_list
                    .add_message(ChatMessage::system(self.current_message_id, msg));
                self.current_message_id += 1;
                self.mark_dirty();
            }
            AgentEvent::PlanApprovalRequest {
                plan_content,
                plan_file,
            } => {
                // TODO: Implement PlanApprovalWidget UI
                // For now, just display a message
                let msg = format!(
                    "📝 Plan approval requested\nFile: {}\nContent length: {} chars",
                    plan_file.display(),
                    plan_content.len()
                );
                self.message_list
                    .add_message(ChatMessage::system(self.current_message_id, msg));
                self.current_message_id += 1;
                self.mark_dirty();
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

        // Start loading and let AgentRunner emit AssistantStart.
        self.is_loading = true;
        self.streaming_start_time = None;
        self.stream_receiver = Some(self.agent.start_turn(text));
        self.mark_dirty();
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
        // Directly render message list without outer border (log-stream style)
        self.message_list.render(frame, area);
    }

    /// Render status bar (simplified borderless style)
    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let status_text = if self.is_loading {
            if let Some(elapsed) = self.get_elapsed_time() {
                format!("⚙️  Generating... {:.1}s · Messages: {}",
                    elapsed, self.message_list.len())
            } else {
                format!("⚙️  Generating... · Messages: {}", self.message_list.len())
            }
        } else {
            format!("✓ Ready · Messages: {}", self.message_list.len())
        };

        let lines = vec![
            // Top separator line
            Line::from(Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(Color::DarkGray)
            )),
            // Status content
            Line::from(Span::styled(
                status_text,
                Style::default().fg(Color::DarkGray)
            )),
        ];

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
    }

    /// Render input area
    fn render_input(&mut self, frame: &mut Frame, area: Rect) {
        self.input.render(frame, area);
    }
}
