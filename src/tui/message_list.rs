use crate::tui::message::{ChatMessage, MessageRole};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, Paragraph, Wrap},
    Frame,
};
use textwrap::wrap;

/// Cache for wrapped text to avoid recomputation
#[derive(Clone)]
struct MessageRenderCache {
    /// The wrapped lines of text
    wrapped_lines: Vec<String>,
    /// The width at which the text was wrapped
    last_width: usize,
    /// Hash of the message content to detect changes
    content_hash: u64,
}

impl MessageRenderCache {
    fn new() -> Self {
        Self {
            wrapped_lines: Vec::new(),
            last_width: 0,
            content_hash: 0,
        }
    }

    /// Check if the cache is still valid for the given width and content
    fn is_valid(&self, width: usize, content: &str) -> bool {
        if self.last_width != width {
            return false;
        }

        // Simple hash using content length and first/last chars
        let hash = Self::simple_hash(content);
        self.content_hash == hash
    }

    /// Simple hash function for content
    fn simple_hash(content: &str) -> u64 {
        let bytes = content.as_bytes();
        let len = bytes.len() as u64;
        let first = bytes.first().copied().unwrap_or(0) as u64;
        let last = bytes.last().copied().unwrap_or(0) as u64;
        (len << 16) | (first << 8) | last
    }

    /// Update the cache with new wrapped lines
    fn update(&mut self, wrapped_lines: Vec<String>, width: usize, content: &str) {
        self.wrapped_lines = wrapped_lines;
        self.last_width = width;
        self.content_hash = Self::simple_hash(content);
    }
}

/// Message list component with scrolling support
pub struct MessageList {
    messages: Vec<ChatMessage>,
    scroll_offset: u16,
    viewport_height: u16,
    viewport_width: u16,
    auto_scroll: bool,
    /// Cache for wrapped text (one per message)
    render_cache: Vec<MessageRenderCache>,
}

impl MessageList {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            viewport_height: 0,
            viewport_width: 0,
            auto_scroll: true,
            render_cache: Vec::new(),
        }
    }

    /// Add a new message to the list
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.render_cache.push(MessageRenderCache::new());
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
    fn calculate_message_height(&mut self, message_idx: usize, width: u16) -> u16 {
        let border_height = 2; // Top and bottom borders
        let role_header_lines = 1u16;

        // Calculate wrapped lines using cache
        let content_width = width.saturating_sub(8); // Match render_message (borders + padding + indent)
        let wrapped_lines = self.get_wrapped_text_cached(message_idx, content_width as usize);

        let content_lines = wrapped_lines.len().max(1) as u16;

        border_height + role_header_lines + content_lines
    }

    /// Get wrapped text using cache (optimized)
    fn get_wrapped_text_cached(&mut self, message_idx: usize, max_width: usize) -> Vec<String> {
        if message_idx >= self.messages.len() {
            return vec![String::new()];
        }

        // Check cache validity first
        let message_content = self.messages[message_idx].content.clone();
        let cache_valid = self.render_cache[message_idx].is_valid(max_width, &message_content);

        if cache_valid {
            return self.render_cache[message_idx].wrapped_lines.clone();
        }

        // Cache miss - compute wrapped text
        let wrapped = self.wrap_text(&message_content, max_width);

        // Update cache
        self.render_cache[message_idx].update(wrapped.clone(), max_width, &message_content);

        wrapped
    }

    /// Wrap text to fit within the given width (uncached)
    fn wrap_text(&self, text: &str, max_width: usize) -> Vec<String> {
        if text.is_empty() {
            return vec![String::new()];
        }

        let max_width = max_width.max(10); // Minimum width to prevent issues

        // Split by newlines first, then wrap each line individually
        text.lines()
            .flat_map(|line| {
                if line.is_empty() {
                    // Preserve empty lines for formatting
                    vec![String::new()]
                } else {
                    // Wrap non-empty lines
                    wrap(line, max_width)
                        .into_iter()
                        .map(|cow| cow.to_string())
                        .collect::<Vec<String>>()
                }
            })
            .collect()
    }

    /// Calculate total height of all messages
    fn calculate_total_height(&mut self, width: u16) -> u16 {
        let mut total = 0;
        for i in 0..self.messages.len() {
            total += self.calculate_message_height(i, width);
            total += 2; // Increased spacing between messages for better visual separation
        }
        total.saturating_sub(2) // Remove last spacing
    }

    /// Render the message list
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.viewport_height = area.height;
        self.viewport_width = area.width;

        if self.messages.is_empty() {
            return;
        }

        let width = area.width;
        let mut current_y = 0u16;
        let visible_start = self.scroll_offset;
        let visible_end = visible_start + area.height;

        // Calculate positions for all messages
        let mut message_positions: Vec<(u16, u16)> = Vec::new();
        for i in 0..self.messages.len() {
            let height = self.calculate_message_height(i, width);
            message_positions.push((current_y, height));
            current_y += height + 2; // +2 for increased spacing between messages
        }

        // Render only visible messages
        for i in 0..self.messages.len() {
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

                self.render_message(frame, i, msg_area);
            }
        }

        // Update scroll offset for auto-scroll
        if self.auto_scroll {
            let total_height = self.calculate_total_height(width);
            self.scroll_offset = total_height.saturating_sub(area.height);
        }
    }

    /// Render a single message with border
    fn render_message(&mut self, frame: &mut Frame, message_idx: usize, area: Rect) {
        let message = &self.messages[message_idx];
        let (border_color, border_type, role_text, role_emoji) = match message.role {
            MessageRole::User => (
                Color::LightCyan,     // Bright cyan for better visibility
                BorderType::Rounded,  // Unified rounded borders
                "You",
                "👤"                  // User icon
            ),
            MessageRole::Assistant => (
                Color::LightGreen,    // Bright green for better visibility
                BorderType::Rounded,  // Unified rounded borders
                "AI",                 // Simplified label
                "🤖"                  // AI icon
            ),
            MessageRole::System => (
                Color::LightBlue,     // Light blue instead of gray for better visibility
                BorderType::Rounded,  // Unified rounded borders
                "System",
                "ℹ️"                  // Info icon
            ),
            MessageRole::Error => (
                Color::LightRed,      // Softer red for less eye strain
                BorderType::Rounded,  // Unified rounded borders
                "Error",
                "⚠️"                  // Warning icon
            ),
        };

        // Add cursor indicator for streaming messages
        // Also trim leading/trailing whitespace to avoid rendering empty lines at start/end
        let content = if !message.is_complete {
            if message.content.is_empty() {
                "⋯".to_string()  // Show ellipsis while waiting for content
            } else {
                let trimmed = message.content.trim();
                format!("{} ▌", trimmed)  // Half-block cursor with space, more subtle
            }
        } else {
            message.content.trim().to_string()
        };

        // Wrap content using cache
        let content_width = area.width.saturating_sub(8) as usize; // Account for borders, padding, and indentation

        // For streaming messages with changing content, use direct wrapping
        // For complete messages, use cache
        let wrapped = if !message.is_complete {
            self.wrap_text(&content, content_width)
        } else {
            self.get_wrapped_text_cached(message_idx, content_width)
        };

        // Create text with role header and indented content
        let mut lines = Vec::new();
        
        // First line: emoji + role name (separate from content)
        let prefix_style = Style::default()
            .fg(border_color)
            .add_modifier(Modifier::BOLD);
        
        lines.push(Line::from(vec![
            Span::raw(" "),  // Left padding
            Span::raw(role_emoji),
            Span::raw(" "),
            Span::styled(role_text, prefix_style),
        ]));
        
        // Content lines: indented for visual hierarchy
        for line in wrapped.iter() {
            lines.push(Line::from(format!("  {}", line)));  // 2-space indent
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
        let total_height = self.calculate_total_height(self.viewport_width.max(1));
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
        if self.viewport_height == 0 || self.viewport_width == 0 {
            self.scroll_offset = 0;
            return;
        }

        let total_height = self.calculate_total_height(self.viewport_width.max(1));
        if total_height > self.viewport_height {
            self.scroll_offset = total_height - self.viewport_height;
        } else {
            self.scroll_offset = 0;
        }
    }

    /// Check if currently at the bottom
    #[allow(dead_code)]
    pub fn is_at_bottom(&mut self) -> bool {
        let total_height = self.calculate_total_height(self.viewport_width.max(1));
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
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clear all messages
    #[allow(dead_code)]
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
