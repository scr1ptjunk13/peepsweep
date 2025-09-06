use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;
use crate::tui::app::App;

pub async fn handle_events(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => {
                    app.quit();
                }
                KeyCode::Tab => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        app.previous_input();
                    } else {
                        app.next_input();
                    }
                }
                KeyCode::Enter => {
                    match app.input_mode {
                        crate::tui::app::InputMode::Chain => {
                            app.toggle_chain_dropdown();
                        },
                        crate::tui::app::InputMode::TokenFrom | crate::tui::app::InputMode::TokenTo => {
                            app.toggle_token_suggestions();
                        },
                        _ => {
                            if app.can_fetch_quotes() {
                                app.fetch_quotes().await;
                            }
                        }
                    }
                }
                KeyCode::Up => {
                    // Handle dropdown/suggestion navigation
                }
                KeyCode::Down => {
                    // Handle dropdown/suggestion navigation
                }
                KeyCode::Char(c) => {
                    match app.input_mode {
                        crate::tui::app::InputMode::Chain => {
                            // Handle number keys for chain selection
                            if let Some(digit) = c.to_digit(10) {
                                if digit >= 1 && digit <= 6 {
                                    app.select_chain((digit - 1) as usize);
                                    app.next_input();
                                }
                            }
                        },
                        _ => {
                            app.add_char(c);
                        }
                    }
                }
                KeyCode::Backspace => {
                    app.delete_char();
                }
                KeyCode::Left => {
                    app.move_cursor_left();
                }
                KeyCode::Right => {
                    app.move_cursor_right();
                }
                _ => {}
            }
        }
    }
    Ok(())
}
