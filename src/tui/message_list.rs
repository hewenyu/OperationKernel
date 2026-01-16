use crate::tui::message::{ChatMessage, MessageRole};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, Paragraph, Wrap},
    Frame,
};
use textwrap::wrap;

/// Message list component with scrolling support
pub struct MessageList {
    messages: Vec<ChatMessage>,
    scroll_offset: u16,
    viewport_height: u16,
    auto_scroll: bool,
}

impl MessageList {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            viewport_height: 0,
            auto_scroll: true,
        }
    }

    /// Add a new message to the list
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        if self.auto_scroll {
            self.auto_scroll_to_bottom();
        }
    }

    /// Get the current streaming message (last incomplete message)
    pub fn get_current_streaming_mut(&mut self) -> Option<&mut ChatMessage> {
        self.messages
            .iter_mut()
            .rev()
            .find(|msg| !msg.is_complete)
    }

    /// Calculate the height of a message when rendered
    fn calculate_message_height(&self, message: &ChatMessage, width: u16) -> u16 {
        let border_height = 2; // Top and bottom borders
        let padding = 1; // Space for role prefix

        // Calculate wrapped lines
        let content_width = width.saturating_sub(4); // Account for borders and padding
        let wrapped_lines = self.wrap_text(&message.content, content_width as usize);

        let content_lines = wrapped_lines.len().max(1) as u16;

        border_height + padding + content_lines
    }

    /// Wrap text to fit within the given width
    fn wrap_text(&self, text: &str, max_width: usize) -> Vec<String> {
        if text.is_empty() {
            return vec![String::new()];
        }

        // Use textwrap for proper word wrapping
        let max_width = max_width.max(10); // Minimum width to prevent issues
        wrap(text, max_width)
            .into_iter()
            .map(|cow| cow.to_string())
            .collect()
    }

    /// Calculate total height of all messages
    fn calculate_total_height(&self, width: u16) -> u16 {
        let mut total = 0;
        for msg in &self.messages {
            total += self.calculate_message_height(msg, width);
            total += 1; // Spacing between messages
        }
        total.saturating_sub(1) // Remove last spacing
    }

    /// Render the message list
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.viewport_height = area.height;

        if self.messages.is_empty() {
            return;
        }

        let width = area.width;
        let mut current_y = 0u16;
        let visible_start = self.scroll_offset;
        let visible_end = visible_start + area.height;

        // Calculate positions for all messages
        let mut message_positions: Vec<(u16, u16)> = Vec::new();
        for msg in &self.messages {
            let height = self.calculate_message_height(msg, width);
            message_positions.push((current_y, height));
            current_y += height + 1; // +1 for spacing
        }

        // Render only visible messages
        for (i, msg) in self.messages.iter().enumerate() {
            let (pos, height) = message_positions[i];

            // Check if message is in visible range
            if pos + height >= visible_start && pos < visible_end {
                // Calculate rendering position
                let render_y = pos.saturating_sub(visible_start);

                // Create the render area
                let msg_area = Rect {
                    x: area.x,
                    y: area.y + render_y,
                    width,
                    height: height.min(area.height.saturating_sub(render_y)),
                };

                self.render_message(frame, msg, msg_area);
            }
        }

        // Update scroll offset for auto-scroll
        if self.auto_scroll {
            let total_height = self.calculate_total_height(width);
            if total_height > area.height {
                self.scroll_offset = total_height - area.height;
            }
        }
    }

    /// Render a single message with border
    fn render_message(&self, frame: &mut Frame, message: &ChatMessage, area: Rect) {
        let (border_color, border_type, role_text) = match message.role {
            MessageRole::User => (Color::Cyan, BorderType::Rounded, "You"),
            MessageRole::Assistant => (Color::Green, BorderType::Double, "Assistant"),
            MessageRole::System => (Color::Gray, BorderType::Plain, "System"),
            MessageRole::Error => (Color::Red, BorderType::Thick, "Error"),
        };

        // Add cursor indicator for streaming messages
        let content = if !message.is_complete {
            format!("{}▊", message.content)
        } else {
            message.content.clone()
        };

        // Wrap content
        let content_width = area.width.saturating_sub(4) as usize;
        let wrapped = self.wrap_text(&content, content_width);

        // Create text with role prefix on first line
        let mut lines = Vec::new();
        for (i, line) in wrapped.iter().enumerate() {
            if i == 0 {
                // First line with role prefix
                let prefix_style = Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD);
                lines.push(Line::from(vec![
                    Span::styled(format!("{}: ", role_text), prefix_style),
                    Span::raw(line),
                ]));
            } else {
                // Continuation lines
                lines.push(Line::from(line.as_str()));
            }
        }

        let text = Text::from(lines);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .border_type(border_type);

        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    /// Scroll down by a number of lines
    pub fn scroll_down(&mut self, lines: u16) {
        let total_height = self.calculate_total_height(self.viewport_height.max(1));
        let max_scroll = total_height.saturating_sub(self.viewport_height);

        if self.scroll_offset < max_scroll {
            self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);

            // Check if we've scrolled to the bottom
            if self.scroll_offset >= max_scroll {
                self.auto_scroll = true;
            }
        }
    }

    /// Scroll up by a number of lines
    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);

        // Disable auto-scroll when user manually scrolls up
        self.auto_scroll = false;
    }

    /// Scroll to the bottom
    fn auto_scroll_to_bottom(&mut self) {
        let total_height = self.calculate_total_height(self.viewport_height.max(1));
        if total_height > self.viewport_height {
            self.scroll_offset = total_height - self.viewport_height;
        } else {
            self.scroll_offset = 0;
        }
    }

    /// Check if currently at the bottom
    pub fn is_at_bottom(&self) -> bool {
        let total_height = self.calculate_total_height(self.viewport_height.max(1));
        if total_height <= self.viewport_height {
            return true;
        }
        let max_scroll = total_height - self.viewport_height;
        self.scroll_offset >= max_scroll
    }

    /// Enable auto-scroll
    pub fn enable_auto_scroll(&mut self) {
        self.auto_scroll = true;
        self.auto_scroll_to_bottom();
    }

    /// Get the number of messages
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }
}

impl Default for MessageList {
    fn default() -> Self {
        Self::new()
    }
}
