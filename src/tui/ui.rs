use ratatui::layout::{Layout, Constraint, Direction, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem, BorderType, Clear};
use ratatui::style::{Style, Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::tui::AppState;

pub fn draw_ui(f: &mut Frame, state: &mut AppState) {
    let size = f.size();

    // Vertically split: Header (3), Main Area (rest - 3), Footer (3)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(size);

    // 1. Render Header
    let header_text = vec![
        Span::styled(" VALVE SERVER MANAGER ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" |  Control game server assignment by blocking regions"),
    ];
    let header_paragraph = Paragraph::new(Line::from(header_text))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(header_paragraph, chunks[0]);

    // 2. Render Main Area (split into Left List and Right Details)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45), // List of servers
            Constraint::Percentage(55), // Details
        ])
        .split(chunks[1]);

    // Gather ping results
    let ping_results = state.pinger.get_results();

    // Prepare list items
    let mut list_items = Vec::new();
    for (i, (code, _region)) in state.selectable_pops.iter().enumerate() {
        let is_selected = i == state.selected_index;
        let is_blocked = state.blocked_pops.contains(code);
        let pop = state.pops.get(code).unwrap();
        let ping_val = ping_results.get(code).copied().flatten();

        // Icon for status
        let status_span = if is_blocked {
            Span::styled(" [BLOCKED] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" [ALLOWED] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        };

        // Ping string
        let ping_span = match ping_val {
            Some(ms) => {
                let color = if ms < 60 {
                    Color::Green
                } else if ms < 150 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                Span::styled(format!(" {} ms", ms), Style::default().fg(color))
            }
            None => Span::styled(" ---", Style::default().fg(Color::DarkGray)),
        };

        // Format selector arrow
        let prefix = if is_selected { "> " } else { "  " };
        let prefix_style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let line_spans = vec![
            Span::styled(prefix, prefix_style),
            status_span,
            Span::styled(format!("{:<3} - {:<20}", code, pop.desc.split('(').next().unwrap_or(&pop.desc).trim()), Style::default().fg(Color::White)),
            ping_span,
        ];

        // Highlight selected line background
        let style = if is_selected {
            Style::default().bg(Color::Rgb(40, 40, 40))
        } else {
            Style::default()
        };

        list_items.push(ListItem::new(Line::from(line_spans)).style(style));
    }

    let list = List::new(list_items)
        .block(Block::default().title(" Relay Regions (Use ↑/↓, Space to Toggle) ").borders(Borders::ALL));
    f.render_widget(list, main_chunks[0]);

    // Render Right Panel (Details)
    if !state.selectable_pops.is_empty() {
        let (selected_code, selected_region) = &state.selectable_pops[state.selected_index];
        let pop = state.pops.get(selected_code).unwrap();
        let is_blocked = state.blocked_pops.contains(selected_code);
        let ping_val = ping_results.get(selected_code).copied().flatten();

        let mut details_lines = Vec::new();
        details_lines.push(Line::from(vec![
            Span::styled("Region: ", Style::default().fg(Color::Gray)),
            Span::styled(*selected_region, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        details_lines.push(Line::from(vec![
            Span::styled("Location: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{} ({})", pop.desc, selected_code), Style::default().fg(Color::White)),
        ]));
        
        let status_text = if is_blocked {
            Span::styled("BLOCKED", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("ALLOWED", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        };
        details_lines.push(Line::from(vec![
            Span::styled("Firewall Status: ", Style::default().fg(Color::Gray)),
            status_text,
        ]));

        details_lines.push(Line::default());
        details_lines.push(Line::from(Span::styled("IP Subnets (SDR Relays):", Style::default().fg(Color::Cyan))));
        if let Some(relays) = &pop.relays {
            for relay in relays {
                details_lines.push(Line::from(Span::styled(format!("  - {}", relay.ipv4), Style::default().fg(Color::White))));
            }
        }

        details_lines.push(Line::default());
        details_lines.push(Line::from(Span::styled("Latency (Live):", Style::default().fg(Color::Cyan))));
        match ping_val {
            Some(ms) => {
                // Render latency bar graph
                // Let 1 bar represent 10ms, max 30 bars
                let bars_count = (ms / 10).min(30) as usize;
                let color = if ms < 60 {
                    Color::Green
                } else if ms < 150 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                
                let fill_bars = "|".repeat(bars_count);
                let empty_spaces = " ".repeat(30 - bars_count);
                details_lines.push(Line::from(vec![
                    Span::raw("  ["),
                    Span::styled(fill_bars, Style::default().fg(color)),
                    Span::raw(empty_spaces),
                    Span::raw(format!("] {} ms", ms)),
                ]));
            }
            None => {
                details_lines.push(Line::from(Span::styled("  [                              ] Ping checking...", Style::default().fg(Color::DarkGray))));
            }
        }

        let details_paragraph = Paragraph::new(details_lines)
            .block(Block::default().title(" Server Metadata ").borders(Borders::ALL));
        f.render_widget(details_paragraph, main_chunks[1]);
    } else {
        let details_paragraph = Paragraph::new("No servers available.")
            .block(Block::default().title(" Server Metadata ").borders(Borders::ALL));
        f.render_widget(details_paragraph, main_chunks[1]);
    }

    // 3. Render Footer Help Legend
    let footer_text = vec![
        Span::styled(" Space", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(": Toggle Block  |  "),
        Span::styled("R", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(": Reset (Unblock All)  |  "),
        Span::styled("S", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(": Settings  |  "),
        Span::styled("Q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(": Quit"),
    ];
    let footer_paragraph = Paragraph::new(Line::from(footer_text))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(footer_paragraph, chunks[2]);

    // 4. Render Settings Overlay if open
    if state.show_settings {
        let block = Block::default()
            .title(" Settings Configuration ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double);
        
        let area = centered_rect(60, 40, size);
        f.render_widget(Clear, area); // clear background under popup

        let persist_icon = if state.settings.persist_rules_on_exit { "[x]" } else { "[ ]" };
        let persist_style = if state.settings.persist_rules_on_exit {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let settings_lines = vec![
            Line::default(),
            Line::from(vec![
                Span::styled(format!("  {} ", persist_icon), persist_style),
                Span::styled("Persist firewall rules on exit", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]),
            Line::default(),
            Line::from(Span::styled("     When enabled, blocks remain active in the system firewall", Style::default().fg(Color::Gray))),
            Line::from(Span::styled("     even after closing this application. You can launch VSM", Style::default().fg(Color::Gray))),
            Line::from(Span::styled("     later to reconfigure or clear rules.", Style::default().fg(Color::Gray))),
            Line::default(),
            Line::from(Span::styled("     When disabled, all firewall rules will be cleaned up", Style::default().fg(Color::Gray))),
            Line::from(Span::styled("     and unblocked automatically when exiting.", Style::default().fg(Color::Gray))),
            Line::default(),
            Line::default(),
            Line::from(Span::styled("  [Space / Enter]: Toggle Setting  |  [S / Esc / Q]: Close Settings", Style::default().fg(Color::Yellow))),
        ];

        let settings_paragraph = Paragraph::new(settings_lines).block(block);
        f.render_widget(settings_paragraph, area);
    }
}

/// Helper function to generate a centered rectangle for popup overlays
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
