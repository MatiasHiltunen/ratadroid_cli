//! Button widget for Ratatui - A clickable button implementation
//! Supports both keyboard navigation and mouse/touch interaction

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Clone, Debug)]
pub struct Button {
    pub label: String,
    pub focused: bool,
    pub clicked: bool,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            focused: false,
            clicked: false,
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn set_clicked(&mut self, clicked: bool) {
        self.clicked = clicked;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Debug: Log button area dimensions
        log::info!("Button '{}' area: {}x{} (x:{}, y:{})", self.label, area.width, area.height, area.x, area.y);
        
        let style = if self.clicked {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if self.focused {
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Gray) // Use Gray instead of DarkGray for better visibility
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::White)
                .bg(Color::Black) // Use Black instead of DarkGray for better contrast
        };

        let border_style = if self.focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let button = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", self.label), style),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .style(style);

        frame.render_widget(button, area);
    }

    /// Check if a mouse/touch event is within this button's area
    pub fn contains(&self, x: u16, y: u16, area: Rect) -> bool {
        x >= area.x
            && x < area.x + area.width
            && y >= area.y
            && y < area.y + area.height
    }
}

