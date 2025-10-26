use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Table, Row},
    Frame,
};
use crate::types::Chain;
use crate::tui::app::{App, InputMode};

pub fn render_ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(8),  // Input form
            Constraint::Min(10),    // Results
            Constraint::Length(3),  // Help
        ])
        .split(f.size());

    // Title
    render_title(f, chunks[0]);
    
    // Input form
    render_input_form(f, chunks[1], app);
    
    // Results
    render_results(f, chunks[2], app);
    
    // Help
    render_help(f, chunks[3], app);
}

fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new("PeepSweep - Multi-Chain DEX Aggregator")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn render_input_form(f: &mut Frame, area: Rect, app: &App) {
    let form_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // Chain
            Constraint::Percentage(25), // Token From
            Constraint::Percentage(25), // Amount
            Constraint::Percentage(25), // Token To
        ])
        .split(area);

    // Chain selection
    let chain_text = if let Some(chain) = &app.selected_chain {
        format!("{:?}", chain)
    } else {
        "Select Chain (1-6)".to_string()
    };
    
    let chain_style = if matches!(app.input_mode, InputMode::Chain) {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    
    let chain_widget = Paragraph::new(chain_text)
        .style(chain_style)
        .block(Block::default().borders(Borders::ALL).title("Chain"));
    f.render_widget(chain_widget, form_chunks[0]);

    // Token From
    let token_from_style = if matches!(app.input_mode, InputMode::TokenFrom) {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    
    let token_from_widget = Paragraph::new(app.token_from.as_str())
        .style(token_from_style)
        .block(Block::default().borders(Borders::ALL).title("From Token"));
    f.render_widget(token_from_widget, form_chunks[1]);

    // Amount
    let amount_style = if matches!(app.input_mode, InputMode::AmountFrom) {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    
    let amount_widget = Paragraph::new(app.amount_from.as_str())
        .style(amount_style)
        .block(Block::default().borders(Borders::ALL).title("Amount"));
    f.render_widget(amount_widget, form_chunks[2]);

    // Token To
    let token_to_style = if matches!(app.input_mode, InputMode::TokenTo) {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    
    let token_to_widget = Paragraph::new(app.token_to.as_str())
        .style(token_to_style)
        .block(Block::default().borders(Borders::ALL).title("To Token"));
    f.render_widget(token_to_widget, form_chunks[3]);

    // Show chain dropdown when active
    if app.show_chain_dropdown && matches!(app.input_mode, InputMode::Chain) {
        render_chain_dropdown(f, form_chunks[0], app);
    }
    
    // Show token suggestions when active
    if app.show_token_suggestions && matches!(app.input_mode, InputMode::TokenFrom) {
        render_token_suggestions(f, form_chunks[1], app);
    } else if app.show_token_suggestions && matches!(app.input_mode, InputMode::TokenTo) {
        render_token_suggestions(f, form_chunks[3], app);
    }
}

fn render_chain_dropdown(f: &mut Frame, field_area: Rect, app: &App) {
    let dropdown_area = Rect {
        x: field_area.x,
        y: field_area.y + field_area.height,
        width: field_area.width,
        height: std::cmp::min(8, app.available_chains.len() as u16 + 2),
    };
    
    let chain_items: Vec<ListItem> = app.available_chains
        .iter()
        .enumerate()
        .map(|(i, chain)| {
            ListItem::new(format!("{}. {:?}", i + 1, chain))
        })
        .collect();
    
    let chain_list = List::new(chain_items)
        .block(Block::default().borders(Borders::ALL).title("Select Chain"))
        .style(Style::default().fg(Color::Cyan));
    
    f.render_widget(chain_list, dropdown_area);
}

fn render_token_suggestions(f: &mut Frame, field_area: Rect, app: &App) {
    let suggestions_area = Rect {
        x: field_area.x,
        y: field_area.y + field_area.height,
        width: field_area.width,
        height: std::cmp::min(6, app.token_suggestions.len() as u16 + 2),
    };
    
    let token_items: Vec<ListItem> = app.token_suggestions
        .iter()
        .take(5) // Show top 5 suggestions
        .map(|token| {
            ListItem::new(token.clone())
        })
        .collect();
    
    let token_list = List::new(token_items)
        .block(Block::default().borders(Borders::ALL).title("Token Suggestions"))
        .style(Style::default().fg(Color::Green));
    
    f.render_widget(token_list, suggestions_area);
}

fn render_results(f: &mut Frame, area: Rect, app: &App) {
    if app.loading {
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Fetching Quotes"))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(50);
        f.render_widget(gauge, area);
        return;
    }

    if let Some(error) = &app.error_message {
        let error_widget = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title("Error"))
            .alignment(Alignment::Center);
        f.render_widget(error_widget, area);
        return;
    }

    if app.quotes.is_empty() {
        let placeholder = Paragraph::new("Enter swap details and press Enter to fetch quotes")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("DEX Quotes"))
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
        return;
    }

    // Create table with quotes
    let quotes_block = Block::default()
        .title("Multi-Chain DEX Aggregator - Real-Time Quotes")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let header = Row::new(vec!["Rank", "DEX", "Output Amount", "Gas Est.", "Slippage", "Impact", "Status"])
        .style(Style::default().fg(Color::Yellow))
        .height(1);

    let rows: Vec<Row> = app.quotes.iter().enumerate().map(|(i, quote)| {
        let (style, rank_symbol) = match i {
            0 => (Style::default().fg(Color::Green).add_modifier(Modifier::BOLD), "#1".to_string()), // Best quote
            1 => (Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD), "#2".to_string()), // Second best
            2 => (Style::default().fg(Color::Magenta), "#3".to_string()), // Third best
            _ => (Style::default().fg(Color::White), format!("#{}", i + 1)), // Others
        };
        
        let status = if i < 3 { "TOP" } else { "OK" };
        
        Row::new(vec![
            rank_symbol,
            quote.dex_name.clone(),
            format!("{} {}", quote.output_amount, app.token_to),
            format!("{} gas", quote.gas_estimate),
            format!("{:.2}%", quote.slippage),
            format!("{:.2}%", quote.price_impact),
            status.to_string(),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, &[
            Constraint::Percentage(8),   // Rank
            Constraint::Percentage(25),  // DEX
            Constraint::Percentage(20),  // Output Amount
            Constraint::Percentage(12),  // Gas Est.
            Constraint::Percentage(10),  // Slippage
            Constraint::Percentage(10),  // Impact
            Constraint::Percentage(15),  // Status
        ])
        .header(header)
        .block(quotes_block);

    f.render_widget(table, area);
}

fn render_help(f: &mut Frame, area: Rect, app: &App) {
    let help_text = match app.input_mode {
        InputMode::Chain => "Enter: Show Chain Options | Tab: Next Field | Numbers 1-6: Select Chain | Esc: Quit",
        InputMode::TokenFrom | InputMode::TokenTo => {
            if app.selected_chain == Some(Chain::Optimism) || app.selected_chain == Some(Chain::Base) {
                "Enter: Show Token Suggestions | Type token symbol | Tab: Next Field | Esc: Quit"
            } else {
                "Type token symbol (USDC, WETH, etc.) | Tab: Next Field | Esc: Quit"
            }
        },
        InputMode::AmountFrom => {
            if app.can_fetch_quotes() {
                "Type amount | Enter: Fetch Quotes | Tab: Next Field | Esc: Quit"
            } else {
                "Type amount | Tab: Next Field | Fill all fields to fetch quotes | Esc: Quit"
            }
        }
    };
    
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
