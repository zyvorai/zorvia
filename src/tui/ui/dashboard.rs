// Dashboard View - Overview of VMs and quick stats

use crate::tui::{colors::tui as colors, config::TuiConfig, state::AppState};
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
    let header_text = Line::from(vec![
        Span::styled("Zorvia".to_string(), Style::default().fg(colors::ORANGE).add_modifier(Modifier::BOLD)),
        Span::styled(" - ".to_string(), Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("KubeVirt VM Manager".to_string(), Style::default().fg(colors::TEXT)),
        Span::styled("  │  ".to_string(), Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("📊 Dashboard".to_string(), Style::default().fg(colors::LIGHT_ORANGE).add_modifier(Modifier::BOLD)),
        Span::styled(": System Overview".to_string(), Style::default().fg(colors::TEXT)),
    ]);
    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));
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

    // Calculate health icon
    let health_icon = if stats.running == 0 && stats.total > 0 {
        "🔴"
    } else if stats.running > 0 {
        "🟢"
    } else {
        "⚪"
    };

    let health_label = format!("{} Health:        ", health_icon);
    let total_vms = format!("{}", stats.total);
    let running_vms = format!("{}", stats.running);
    let stopped_vms = format!("{}", stats.stopped);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("💻 Total VMs:     ".to_string(), Style::default().fg(colors::TEXT)),
            Span::styled(
                total_vms,
                Style::default().fg(colors::LIGHT_ORANGE).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("🟢 Running:       ".to_string(), Style::default().fg(colors::TEXT)),
            Span::styled(
                running_vms,
                Style::default().fg(colors::SUCCESS).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("⏸  Stopped:       ".to_string(), Style::default().fg(colors::TEXT)),
            Span::styled(
                stopped_vms,
                Style::default().fg(colors::TEXT_MUTED).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(health_label, Style::default().fg(colors::TEXT)),
            Span::styled(
                if stats.running > 0 { "Healthy".to_string() } else if stats.total > 0 { "All Stopped".to_string() } else { "No VMs".to_string() },
                Style::default().fg(if stats.running > 0 { colors::SUCCESS } else { colors::TEXT_MUTED }).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("🌐 Namespace:     ".to_string(), Style::default().fg(colors::TEXT)),
            Span::styled(
                state.namespace.clone(),
                Style::default().fg(colors::LIGHT_ORANGE).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let title = Span::styled("📊 VM Statistics", Style::default().fg(colors::ORANGE).add_modifier(Modifier::BOLD));
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)).title(title))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn render_quick_actions(f: &mut Frame, area: ratatui::layout::Rect) {
    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  1 ", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
            Span::styled("📊  ", Style::default().fg(colors::ORANGE)),
            Span::styled("Dashboard", Style::default().fg(colors::TEXT)),
            Span::styled("  (current)", Style::default().fg(colors::TEXT_MUTED).add_modifier(Modifier::ITALIC)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  2 ", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
            Span::styled("💻  ", Style::default().fg(colors::ORANGE)),
            Span::styled("VM List", Style::default().fg(colors::TEXT)),
            Span::styled("  - Browse & manage VMs", Style::default().fg(colors::TEXT_MUTED)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  3 ", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
            Span::styled("📸  ", Style::default().fg(colors::ORANGE)),
            Span::styled("Snapshots", Style::default().fg(colors::TEXT)),
            Span::styled("  - Backup & restore", Style::default().fg(colors::TEXT_MUTED)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  4 ", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
            Span::styled("⚙️  ", Style::default().fg(colors::ORANGE)),
            Span::styled("Profiles", Style::default().fg(colors::TEXT)),
            Span::styled("  - Resource templates", Style::default().fg(colors::TEXT_MUTED)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  5 ", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
            Span::styled("🏗️  ", Style::default().fg(colors::ORANGE)),
            Span::styled("Blueprints", Style::default().fg(colors::TEXT)),
            Span::styled("  - Multi-VM deployments", Style::default().fg(colors::TEXT_MUTED)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ? ", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
            Span::styled("📖  ", Style::default().fg(colors::ORANGE)),
            Span::styled("Help", Style::default().fg(colors::TEXT)),
            Span::styled("  - Keyboard shortcuts", Style::default().fg(colors::TEXT_MUTED)),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("  💡 Pro Tip: ", Style::default().fg(colors::WARNING).add_modifier(Modifier::BOLD)),
            Span::styled("Press ", Style::default().fg(colors::TEXT_MUTED)),
            Span::styled("Ctrl+P", Style::default().fg(colors::LIGHT_ORANGE).add_modifier(Modifier::BOLD)),
            Span::styled(" for quick jump!", Style::default().fg(colors::TEXT_MUTED)),
        ]),
    ];

    let title = Span::styled("🚀 Quick Navigation", Style::default().fg(colors::ORANGE).add_modifier(Modifier::BOLD));
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)).title(title))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn render_status_bar(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let status_line = Line::from(vec![
        Span::styled("⌨  ", Style::default().fg(colors::ORANGE)),
        Span::styled("1-5", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Jump", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("r", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Refresh", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("?", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Help", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("q", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Quit", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("⏱  ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled(
            format!("Last refresh: {}", state.last_refresh.format("%H:%M:%S")),
            Style::default().fg(colors::TEXT_MUTED).add_modifier(Modifier::ITALIC),
        ),
    ]);

    let status = Paragraph::new(status_line)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));

    f.render_widget(status, area);
}
