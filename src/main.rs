mod config;
mod event;
mod tui;

use crate::event::{Event, EventResult};
use crate::tui::App;
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::time::Duration;
use tokio::time::interval;

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Load or create configuration
    let _config = config::load_or_create_config()?;

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Create app state
    let mut app = App::new();

    // Run the application
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    restore_terminal(&mut terminal)?;

    // Print any error that occurred
    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

/// Setup the terminal for TUI rendering
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the main application loop
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> EventResult<()> {
    // Create event stream
    let mut event_stream = EventStream::new();

    // Create tick interval for periodic updates (60 FPS = ~16ms)
    let mut tick_interval = interval(Duration::from_millis(16));

    loop {
        // Render the UI
        terminal.draw(|frame| app.render(frame))?;

        // Handle events
        tokio::select! {
            // Handle crossterm events (keyboard, mouse, resize)
            maybe_event = event_stream.next() => {
                if let Some(Ok(evt)) = maybe_event {
                    let app_event = match evt {
                        CrosstermEvent::Key(key) => Event::Key(key),
                        CrosstermEvent::Mouse(mouse) => Event::Mouse(mouse),
                        CrosstermEvent::Resize(w, h) => Event::Resize(w, h),
                        _ => continue,
                    };
                    app.handle_event(app_event)?;
                }
            }

            // Handle tick events
            _ = tick_interval.tick() => {
                app.handle_event(Event::Tick)?;
            }
        }

        // Check if we should quit
        if app.should_quit() {
            break;
        }
    }

    Ok(())
}
