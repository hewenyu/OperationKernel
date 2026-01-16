use crossterm::event::{KeyEvent, MouseEvent};

/// Events that can occur in the application
#[derive(Debug, Clone)]
pub enum Event {
    /// Terminal key press event
    Key(KeyEvent),
    /// Terminal mouse event
    Mouse(MouseEvent),
    /// Terminal resize event
    Resize(u16, u16),
    /// Tick event for periodic updates (for animations, etc.)
    Tick,
    /// Request to quit the application
    Quit,
}

/// Result type for event handling
pub type EventResult<T> = anyhow::Result<T>;
