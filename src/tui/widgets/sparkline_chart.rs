// Sparkline Chart Widget - Trend visualization
use crate::tui::colors::tui as colors;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Sparkline},
    Frame,
};

pub struct SparklineChart {
    pub title: String,
    pub data: Vec<u64>,
    pub max_value: Option<u64>,
}

impl SparklineChart {
    pub fn new(title: impl Into<String>, data: Vec<u64>) -> Self {
        Self {
            title: title.into(),
            data,
            max_value: None,
        }
    }

    pub fn with_max(mut self, max: u64) -> Self {
        self.max_value = Some(max);
        self
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let max = self
            .max_value
            .unwrap_or_else(|| *self.data.iter().max().unwrap_or(&1));

        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors::BORDER))
                    .title(Span::styled(
                        &self.title,
                        Style::default()
                            .fg(colors::ORANGE)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .data(&self.data)
            .style(Style::default().fg(colors::LIGHT_ORANGE))
            .max(max);

        f.render_widget(sparkline, area);
    }
}
