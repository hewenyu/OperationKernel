use crate::event::{Event, EventResult};
use crate::tui::InputWidget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Main application state
pub struct App {
    /// Chat history messages
    messages: Vec<String>,
    /// Input widget for user text
    pub input: InputWidget,
    /// Whether the application should quit
    should_quit: bool,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Self {
        Self {
            messages: vec![
                "Welcome to OperationKernel (OK Agent)!".to_string(),
                "Press Enter to send, Shift+Enter for new line, Ctrl+C to quit".to_string(),
            ],
            input: InputWidget::new(),
            should_quit: false,
        }
    }

    /// Check if the application should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
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
            self.submit_message();
            return Ok(());
        }

        // Forward other keys to the input widget (including Shift+Enter)
        self.input.handle_key(key);
        Ok(())
    }

    /// Submit the current message
    fn submit_message(&mut self) {
        let text = self.input.take_text();
        if !text.trim().is_empty() {
            // Add user message to history
            self.messages.push(format!("You: {}", text));

            // Echo response (Phase 1: no LLM yet)
            self.messages.push(format!("Echo: {}", text));
        }
    }

    /// Render the application UI
    pub fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),        // Chat history
                Constraint::Length(3),     // Status bar
                Constraint::Length(5),     // Input area
            ])
            .split(frame.area());

        self.render_chat(frame, chunks[0]);
        self.render_status(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
    }

    /// Render chat history
    fn render_chat(&self, frame: &mut Frame, area: Rect) {
        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .map(|msg| {
                let style = if msg.starts_with("You:") {
                    Style::default().fg(Color::Cyan)
                } else if msg.starts_with("Echo:") {
                    Style::default().fg(Color::Green)
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
        let status_text = vec![
            Line::from(vec![
                Span::styled("Phase 1: ", Style::default().fg(Color::Yellow)),
                Span::raw("Echo Mode"),
                Span::raw(" | "),
                Span::styled("Messages: ", Style::default().fg(Color::Cyan)),
                Span::raw(self.messages.len().to_string()),
            ]),
        ];

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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
