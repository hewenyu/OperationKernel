use crossterm::event::KeyEvent;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};
use tui_textarea::TextArea;

/// Input widget wrapper around tui-textarea
pub struct InputWidget {
    textarea: TextArea<'static>,
}

impl InputWidget {
    /// Create a new input widget
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input (Enter to send, Shift+Enter for new line)")
                .border_style(Style::default().fg(Color::Blue)),
        );
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default());

        Self { textarea }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: KeyEvent) {
        self.textarea.input(key);
    }

    /// Get the current text and clear the input
    pub fn take_text(&mut self) -> String {
        let text = self.textarea.lines().join("\n");
        self.textarea = TextArea::default();
        self.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input (Enter to send, Shift+Enter for new line)")
                .border_style(Style::default().fg(Color::Blue)),
        );
        text
    }

    /// Render the input widget
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(&self.textarea, area);
    }
}

impl Default for InputWidget {
    fn default() -> Self {
        Self::new()
    }
}
