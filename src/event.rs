use crossterm::event::{KeyEvent, MouseEvent};

/// Events that can occur in the application
#[derive(Debug, Clone)]
pub enum Event {
    /// Terminal key press event
    Key(KeyEvent),
    /// Terminal mouse event (reserved for future use)
    #[allow(dead_code)]
    Mouse(MouseEvent),
    /// Terminal resize event (reserved for future use)
    #[allow(dead_code)]
    Resize(u16, u16),
    /// Tick event for periodic updates (for animations, etc.)
    Tick,
    /// Request to quit the application (reserved for future use)
    #[allow(dead_code)]
    Quit,
}

/// Result type for event handling
pub type EventResult<T> = anyhow::Result<T>;
