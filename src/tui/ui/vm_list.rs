// VM List View - Table of VMs with selection and actions

use crate::tui::{config::TuiConfig, state::AppState};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render(f: &mut Frame, state: &mut AppState, _config: &TuiConfig) {
    let size = f.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // VM table
            Constraint::Length(4),  // Help text
        ])
        .split(size);

    // Header
    let header = Paragraph::new(format!("Virtual Machines ({})", state.vms.len()))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // VM table
    render_vm_table(f, state, chunks[1]);

    // Help text
    render_help(f, chunks[2]);
}

fn render_vm_table(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let header_cells = ["Name", "Status", "CPU", "Memory", "Age"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray))
        .height(1);

    let rows = state.vms.iter().enumerate().map(|(i, vm)| {
        let is_selected = i == state.selected_index;

        let status_color = match vm.status.as_str() {
            "Running" => Color::Green,
            "Stopped" => Color::Gray,
            "Starting" | "Pending" => Color::Yellow,
            "Failed" | "Error" => Color::Red,
            _ => Color::White,
        };

        let cells = vec![
            Cell::from(vm.name.clone()),
            Cell::from(vm.status.clone()).style(Style::default().fg(status_color)),
            Cell::from(vm.cpu.clone()),
            Cell::from(vm.memory.clone()),
            Cell::from(vm.age.clone()),
        ];

        let style = if is_selected {
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        Row::new(cells).style(style).height(1)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),  // Name
            Constraint::Percentage(15),  // Status
            Constraint::Percentage(15),  // CPU
            Constraint::Percentage(20),  // Memory
            Constraint::Percentage(25),  // Age
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("VMs"))
    .column_spacing(1);

    f.render_widget(table, area);
}

fn render_help(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from("↑↓/jk: Navigate | Enter: Details | s: Start | x: Stop | d: Delete | q: Quit | ?: Help"),
    ];

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(help, area);
}
