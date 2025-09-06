use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;

mod aggregator;
mod dexes;
mod tui;
mod types;

use crate::tui::{App, render_ui, handle_events};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and initialize aggregator
    let mut app = App::new();
    
    // Initialize aggregator in background (non-blocking)
    if let Err(e) = app.initialize_aggregator().await {
        eprintln!("Warning: Failed to initialize aggregator: {}", e);
    }

    // Main loop
    loop {
        terminal.draw(|f| render_ui(f, &app))?;
        
        handle_events(&mut app).await?;
        
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}