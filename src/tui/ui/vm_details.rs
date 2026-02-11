// VM Details View - Detailed information about selected VM

use crate::tui::{config::TuiConfig, state::AppState};
use crate::tui::colors::tui as colors;
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

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Details
            Constraint::Length(3),  // Help
        ])
        .split(size);

    // Header
    let vm_name = state.selected_vm()
        .map(|vm| vm.name.clone())
        .unwrap_or_else(|| "No VM selected".to_string());

    let header = Paragraph::new(format!("VM Details: {}", vm_name))
        .style(Style::default().fg(colors::ORANGE).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));
    f.render_widget(header, chunks[0]);

    // Details
    if let Some(vm) = state.selected_vm() {
        render_details(f, vm, chunks[1]);
    } else {
        let text = Paragraph::new("No VM selected")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));
        f.render_widget(text, chunks[1]);
    }

    // Help
    let help = Paragraph::new("Backspace: Back to list | q: Quit")
        .style(Style::default().fg(colors::TEXT_MUTED))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));
    f.render_widget(help, chunks[2]);
}

fn render_details(f: &mut Frame, vm: &crate::tui::state::VmInfo, area: ratatui::layout::Rect) {
    let status_color = match vm.status.as_str() {
        "Running" => colors::SUCCESS,
        "Stopped" => colors::TEXT_MUTED,
        _ => colors::WARNING,
    };

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Name:       ", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
            Span::raw(&vm.name),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Status:     ", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
            Span::styled(&vm.status, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("CPU:        ", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
            Span::raw(&vm.cpu),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Memory:     ", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
            Span::raw(&vm.memory),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Age:        ", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
            Span::raw(&vm.age),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Ready:      ", Style::default().fg(colors::TEXT).add_modifier(Modifier::BOLD)),
            Span::styled(
                if vm.ready { "Yes" } else { "No" },
                if vm.ready {
                    Style::default().fg(colors::SUCCESS)
                } else {
                    Style::default().fg(colors::ERROR)
                },
            ),
        ]),
    ];

    let details = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)).title("Details"))
        .alignment(Alignment::Left);

    f.render_widget(details, area);
}
