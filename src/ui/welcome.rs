use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::theme;

pub fn draw(f: &mut Frame) {
    let area = f.area();
    let bounds = centered_rect(60, 5, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border());

    let inner = block.inner(bounds);

    f.render_widget(block, bounds);

    let lines = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "welcome to mnemo",
            theme::title().fg(theme::ACCENT),
        ))),
        lines[0],
    );
    f.render_widget(
        Paragraph::new(
            "spaced repetition flashcards without ever leaving the comfort of your terminal",
        ),
        lines[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "press enter to log in to github",
            theme::dim(),
        ))),
        lines[2],
    );

    let hint_y = area.y + area.height.saturating_sub(1);
    let hint = Paragraph::new("enter continue  ·  q quit").style(theme::hint());
    f.render_widget(hint, Rect::new(area.x, hint_y, area.width, 1));
}

/// Centers a rectangle of the given percent width and vertical row count
/// inside `r`, clamping so it never escapes the parent rect.
pub fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let pop_width = (r.width.saturating_mul(percent_x) / 100).max(1);
    let pop_width = pop_width.min(r.width);
    let pop_height = height.min(r.height).max(3);
    let x = r.x + (r.width - pop_width) / 2;
    let y = r.y + (r.height - pop_height) / 2;
    Rect::new(x, y, pop_width, pop_height)
}
