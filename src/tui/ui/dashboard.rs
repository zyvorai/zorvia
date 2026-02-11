// Dashboard View - Overview of VMs and quick stats

use crate::tui::{config::TuiConfig, state::AppState};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, state: &AppState, _config: &TuiConfig) {
    let size = f.area();

    // Main layout: Header | Content | Status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Content
            Constraint::Length(3),  // Status bar
        ])
        .split(size);

    // Header
    let header = Paragraph::new("Zorvia Dashboard")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Content area - split into stats and quick actions
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),  // Stats panel
            Constraint::Percentage(50),  // Quick actions panel
        ])
        .split(chunks[1]);

    // Stats panel
    render_stats_panel(f, state, content_chunks[0]);

    // Quick actions panel
    render_quick_actions(f, content_chunks[1]);

    // Status bar
    render_status_bar(f, state, chunks[2]);
}

fn render_stats_panel(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let stats = state.get_stats();

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Total VMs:    ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", stats.total),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Running:      ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", stats.running),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Stopped:      ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", stats.stopped),
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Namespace:    ", Style::default().fg(Color::White)),
            Span::styled(
                &state.namespace,
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("VM Statistics"))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn render_quick_actions(f: &mut Frame, area: ratatui::layout::Rect) {
    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("2", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  VM List"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("3", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  Snapshots"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("4", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  Profiles"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("5", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  Blueprints"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("?", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  Help"),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Quick Actions"))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn render_status_bar(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let status_text = format!(
        "Press 'q' to quit | '?' for help | Ctrl+R to refresh | Last refresh: {}",
        state.last_refresh.format("%H:%M:%S")
    );

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}
