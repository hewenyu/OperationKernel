mod agent;
mod config;
mod event;
mod llm;
mod logging;
mod process;
mod tool;
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
    let config = config::load_or_create_config()?;

    // Initialize debug logging (writes to ~/.config/ok/ok-debug.log when enabled)
    let _log_guard = logging::init(&config).unwrap_or_else(|e| {
        eprintln!("Failed to initialize debug logging: {e}");
        None
    });

    // Get default station
    let station = config
        .stations
        .iter()
        .find(|s| s.id == config.default_station)
        .ok_or_else(|| anyhow::anyhow!("Default station '{}' not found", config.default_station))?
        .clone();

    tracing::info!(
        default_station = %config.default_station,
        provider = ?station.provider,
        model = %station.model,
        "starting ok"
    );

    // Create LLM client
    let llm_client = llm::anthropic::AnthropicClient::new(station);

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Create app state
    let mut app = App::new(llm_client);

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
        // Only render if needed (event-driven rendering for performance)
        if app.needs_render() {
            terminal.draw(|frame| {
                let size = frame.area();
                app.update_terminal_size(size.width, size.height);
                app.render(frame);
            })?;
            app.mark_rendered();
        }

        // Handle events
        tokio::select! {
            // Handle crossterm events (keyboard, mouse, resize)
            maybe_event = event_stream.next() => {
                if let Some(Ok(evt)) = maybe_event {
                    let app_event = match evt {
                        CrosstermEvent::Key(key) => Event::Key(key),
                        CrosstermEvent::Mouse(mouse) => Event::Mouse(mouse),
                        CrosstermEvent::Resize(w, h) => {
                            app.update_terminal_size(w, h);
                            Event::Resize(w, h)
                        },
                        _ => continue,
                    };
                    app.handle_event(app_event)?;
                }
            }

            // Handle tick events
            _ = tick_interval.tick() => {
                // Update spinner animation
                app.tick_spinner();

                // Poll for streaming chunks
                while let Some(evt) = app.poll_stream() {
                    app.handle_async_event(evt);
                }
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
