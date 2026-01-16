use crate::event::{Event, EventResult};
use crate::llm::anthropic::AnthropicClient;
use crate::llm::types::{Message, StreamChunk};
use crate::tui::InputWidget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use tokio::sync::mpsc;

/// Main application state
pub struct App {
    /// LLM client
    llm_client: AnthropicClient,
    /// Display messages (for UI)
    display_messages: Vec<String>,
    /// Conversation history (for LLM)
    conversation: Vec<Message>,
    /// Currently streaming assistant message
    current_assistant_message: String,
    /// Input widget for user text
    pub input: InputWidget,
    /// Whether the application should quit
    should_quit: bool,
    /// Whether we're currently waiting for LLM response
    is_loading: bool,
    /// Channel receiver for streaming responses
    stream_receiver: Option<mpsc::UnboundedReceiver<StreamChunk>>,
}

impl App {
    /// Create a new application instance
    pub fn new(llm_client: AnthropicClient) -> Self {
        Self {
            llm_client,
            display_messages: vec![
                "Welcome to OperationKernel (OK Agent)!".to_string(),
                "Connected to Claude API - Ready to chat!".to_string(),
                "Press Enter to send, Shift+Enter for new line, Ctrl+C to quit".to_string(),
            ],
            conversation: Vec::new(),
            current_assistant_message: String::new(),
            input: InputWidget::new(),
            should_quit: false,
            is_loading: false,
            stream_receiver: None,
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
                self.current_assistant_message.push_str(&text);
            }
            StreamChunk::Done => {
                // Finish the current message
                if !self.current_assistant_message.is_empty() {
                    self.display_messages
                        .push(format!("Assistant: {}", self.current_assistant_message));
                    self.conversation
                        .push(Message::assistant(self.current_assistant_message.clone()));
                    self.current_assistant_message.clear();
                }
                self.is_loading = false;
                self.stream_receiver = None;
            }
            StreamChunk::Error(err) => {
                self.display_messages.push(format!("Error: {}", err));
                self.is_loading = false;
                self.stream_receiver = None;
                self.current_assistant_message.clear();
            }
        }
    }

    /// Handle an event
    pub fn handle_event(&mut self, event: Event) -> EventResult<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Quit => {
                self.should_quit = true;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Handle keyboard input
    fn handle_key(&mut self, key: KeyEvent) -> EventResult<()> {
        // Handle Ctrl+C to quit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return Ok(());
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

    /// Submit the current message and start LLM streaming
    fn submit_message(&mut self) {
        let text = self.input.take_text();
        if text.trim().is_empty() {
            return;
        }

        // Add user message
        self.display_messages.push(format!("You: {}", text));
        self.conversation.push(Message::user(text));

        // Start loading
        self.is_loading = true;
        self.display_messages.push("Assistant: ...".to_string());

        // Create a channel for streaming
        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_receiver = Some(rx);

        // Clone what we need for the async task
        let llm_client = self.llm_client.clone();
        let conversation = self.conversation.clone();

        // Spawn async task to call LLM
        tokio::spawn(async move {
            match llm_client.stream_chat(conversation).await {
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
    fn render_chat(&self, frame: &mut Frame, area: Rect) {
        let mut display_messages = self.display_messages.clone();

        // If we're streaming, replace the last "..." with current content
        if self.is_loading && !self.current_assistant_message.is_empty() {
            if let Some(last) = display_messages.last_mut() {
                *last = format!("Assistant: {}", self.current_assistant_message);
            }
        }

        let messages: Vec<ListItem> = display_messages
            .iter()
            .map(|msg| {
                let style = if msg.starts_with("You:") {
                    Style::default().fg(Color::Cyan)
                } else if msg.starts_with("Assistant:") {
                    Style::default().fg(Color::Green)
                } else if msg.starts_with("Error:") {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(Line::from(Span::styled(msg.clone(), style)))
            })
            .collect();

        let chat = List::new(messages).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Chat History")
                .border_style(Style::default().fg(Color::White)),
        );

        frame.render_widget(chat, area);
    }

    /// Render status bar
    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let status_text = vec![Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(if self.is_loading {
                "Generating..."
            } else {
                "Ready"
            }),
            Span::raw(" | "),
            Span::styled("Messages: ", Style::default().fg(Color::Cyan)),
            Span::raw(self.conversation.len().to_string()),
        ])];

        let status = Paragraph::new(status_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Status")
                .border_style(Style::default().fg(Color::White)),
        );

        frame.render_widget(status, area);
    }

    /// Render input area
    fn render_input(&mut self, frame: &mut Frame, area: Rect) {
        self.input.render(frame, area);
    }
}
