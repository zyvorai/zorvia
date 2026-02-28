// Resource Gauge Widget - Visual resource usage indicators
use crate::tui::colors::tui as colors;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Gauge},
    Frame,
};

pub struct ResourceGauge {
    pub label: String,
    pub value: f64,      // 0.0 to 100.0
    pub max: f64,        // Maximum value
    pub unit: String,    // e.g., "GB", "cores", "%"
    pub show_percentage: bool,
}

impl ResourceGauge {
    pub fn new(label: impl Into<String>, value: f64, max: f64, unit: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value,
            max,
            unit: unit.into(),
            show_percentage: true,
        }
    }

    pub fn percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let percentage = if self.max > 0.0 {
            ((self.value / self.max) * 100.0).min(100.0)
        } else {
            0.0
        };

        let color = if percentage >= 90.0 {
            colors::ERROR
        } else if percentage >= 75.0 {
            colors::WARNING
        } else if percentage >= 50.0 {
            colors::ORANGE
        } else {
            colors::SUCCESS
        };

        let label = if self.show_percentage {
            format!("{}: {:.1}/{} {} ({:.0}%)",
                self.label, self.value, self.max, self.unit, percentage)
        } else {
            format!("{}: {:.1}/{} {}",
                self.label, self.value, self.max, self.unit)
        };

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors::BORDER)))
            .gauge_style(
                Style::default()
                    .fg(color)
                    .bg(colors::TEXT_MUTED)
                    .add_modifier(Modifier::BOLD),
            )
            .percent(percentage as u16)
            .label(label);

        f.render_widget(gauge, area);
    }
}

pub struct MultiGaugePanel {
    pub title: String,
    pub gauges: Vec<ResourceGauge>,
}

impl MultiGaugePanel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            gauges: Vec::new(),
        }
    }

    pub fn add_gauge(mut self, gauge: ResourceGauge) -> Self {
        self.gauges.push(gauge);
        self
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::text::Span;

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(colors::BORDER))
            .title(Span::styled(
                &self.title,
                Style::default().fg(colors::ORANGE).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Create constraints for each gauge
        let constraints: Vec<Constraint> = self.gauges
            .iter()
            .map(|_| Constraint::Length(3))
            .collect();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        for (i, gauge) in self.gauges.iter().enumerate() {
            if i < chunks.len() {
                gauge.render(f, chunks[i]);
            }
        }
    }
}
