use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::theme;
use super::welcome::centered_rect;

pub fn draw(f: &mut Frame, message: &str) {
    let area = f.area();
    let bounds = centered_rect(70, 7, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(bounds);
    f.render_widget(block, bounds);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(1),    // message (wraps)
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "login failed",
            theme::title().fg(theme::ACCENT),
        ))),
        rows[0],
    );

    f.render_widget(Paragraph::new(message).wrap(Wrap { trim: true }), rows[1]);

    let hint_y = area.y + area.height.saturating_sub(1);
    let hint = Paragraph::new("r retry  ·  esc back").style(theme::hint());
    f.render_widget(hint, Rect::new(area.x, hint_y, area.width, 1));
}
