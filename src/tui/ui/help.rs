// Help View - Comprehensive keybinding reference

use crate::tui::config::TuiConfig;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, _config: &TuiConfig) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Help content
            Constraint::Length(3),  // Footer
        ])
        .split(size);

    // Header
    let header = Paragraph::new("Zorvia TUI - Help")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Help content
    render_help_content(f, chunks[1]);

    // Footer
    let footer = Paragraph::new("Press any key to return")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_help_content(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("GLOBAL KEYBINDINGS", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q / Esc       ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit application"),
        ]),
        Line::from(vec![
            Span::styled("  ?             ", Style::default().fg(Color::Cyan)),
            Span::raw("Show this help screen"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+R        ", Style::default().fg(Color::Cyan)),
            Span::raw("Refresh data from Kubernetes"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("VIEW NAVIGATION", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  1             ", Style::default().fg(Color::Cyan)),
            Span::raw("Dashboard"),
        ]),
        Line::from(vec![
            Span::styled("  2             ", Style::default().fg(Color::Cyan)),
            Span::raw("VM List"),
        ]),
        Line::from(vec![
            Span::styled("  3             ", Style::default().fg(Color::Cyan)),
            Span::raw("Snapshots"),
        ]),
        Line::from(vec![
            Span::styled("  4             ", Style::default().fg(Color::Cyan)),
            Span::raw("Profiles"),
        ]),
        Line::from(vec![
            Span::styled("  5             ", Style::default().fg(Color::Cyan)),
            Span::raw("Blueprints"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("VM LIST VIEW", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑↓ / j/k      ", Style::default().fg(Color::Cyan)),
            Span::raw("Navigate VM list"),
        ]),
        Line::from(vec![
            Span::styled("  Enter         ", Style::default().fg(Color::Cyan)),
            Span::raw("View VM details"),
        ]),
        Line::from(vec![
            Span::styled("  s             ", Style::default().fg(Color::Cyan)),
            Span::raw("Start selected VM"),
        ]),
        Line::from(vec![
            Span::styled("  x             ", Style::default().fg(Color::Cyan)),
            Span::raw("Stop selected VM"),
        ]),
        Line::from(vec![
            Span::styled("  d             ", Style::default().fg(Color::Cyan)),
            Span::raw("Delete selected VM"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("VM DETAILS VIEW", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Backspace     ", Style::default().fg(Color::Cyan)),
            Span::raw("Return to VM list"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("SNAPSHOTS VIEW", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑↓ / j/k      ", Style::default().fg(Color::Cyan)),
            Span::raw("Navigate snapshot list"),
        ]),
        Line::from(vec![
            Span::styled("  c             ", Style::default().fg(Color::Cyan)),
            Span::raw("Create new snapshot"),
        ]),
        Line::from(vec![
            Span::styled("  d             ", Style::default().fg(Color::Cyan)),
            Span::raw("Delete selected snapshot"),
        ]),
        Line::from(vec![
            Span::styled("  r             ", Style::default().fg(Color::Cyan)),
            Span::raw("Restore selected snapshot"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("PROFILES/BLUEPRINTS VIEW", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑↓ / j/k      ", Style::default().fg(Color::Cyan)),
            Span::raw("Navigate list"),
        ]),
        Line::from(vec![
            Span::styled("  Enter         ", Style::default().fg(Color::Cyan)),
            Span::raw("View details"),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Keybindings"))
        .alignment(Alignment::Left);

    f.render_widget(help, area);
}
