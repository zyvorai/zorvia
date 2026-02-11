// VM List View - Table of VMs with selection and actions

use crate::tui::{colors::tui as colors, config::TuiConfig, state::AppState};
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
    use ratatui::text::Span;
    let vm_count_text = format!(": {} Virtual Machines", state.vms.len());
    let header_text = Line::from(vec![
        Span::styled("Zorvia".to_string(), Style::default().fg(colors::ORANGE).add_modifier(Modifier::BOLD)),
        Span::styled(" - ".to_string(), Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("KubeVirt VM Manager".to_string(), Style::default().fg(colors::TEXT)),
        Span::styled("  │  ".to_string(), Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("💻 VM List".to_string(), Style::default().fg(colors::LIGHT_ORANGE).add_modifier(Modifier::BOLD)),
        Span::styled(vm_count_text, Style::default().fg(colors::TEXT)),
    ]);
    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));
    f.render_widget(header, chunks[0]);

    // VM table
    render_vm_table(f, state, chunks[1]);

    // Help text
    render_help(f, chunks[2]);
}

fn render_vm_table(f: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let header_cells = ["Name", "Status", "CPU", "Memory", "Age"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(colors::LIGHT_ORANGE).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(colors::DARK_ORANGE))
        .height(1);

    let rows = state.vms.iter().enumerate().map(|(i, vm)| {
        let is_selected = i == state.selected_index;

        let (status_color, status_icon) = match vm.status.as_str() {
            "Running" => (colors::SUCCESS, "🟢"),
            "Stopped" => (colors::TEXT_MUTED, "⏸ "),
            "Starting" | "Pending" => (colors::WARNING, "🟡"),
            "Failed" | "Error" => (colors::ERROR, "🔴"),
            _ => (colors::TEXT, "⚪"),
        };

        let name_cell = if is_selected {
            Cell::from(format!("▶ {}", vm.name))
        } else {
            Cell::from(format!("  {}", vm.name))
        };

        let cells = vec![
            name_cell,
            Cell::from(format!("{} {}", status_icon, vm.status)).style(Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            Cell::from(format!("💾 {}", vm.cpu)),
            Cell::from(format!("🧠 {}", vm.memory)),
            Cell::from(format!("⏱  {}", vm.age)),
        ];

        let style = if is_selected {
            Style::default()
                .bg(colors::DARK_ORANGE)
                .fg(colors::TEXT)
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
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)).title("VMs"))
    .column_spacing(1);

    f.render_widget(table, area);
}

fn render_help(f: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::text::Span;
    let help_line = Line::from(vec![
        Span::styled("⌨  ", Style::default().fg(colors::ORANGE)),
        Span::styled("↑↓/jk", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Navigate", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("Enter", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Details", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("s", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Start", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("x", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Stop", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("d", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Delete", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("r", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Refresh", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("?", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Help", Style::default().fg(colors::TEXT)),
        Span::styled(" │ ", Style::default().fg(colors::TEXT_MUTED)),
        Span::styled("q", Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)),
        Span::styled(": Quit", Style::default().fg(colors::TEXT)),
    ]);

    let help_text = vec![
        Line::from(""),
        help_line,
    ];

    let help = Paragraph::new(help_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)));

    f.render_widget(help, area);
}
