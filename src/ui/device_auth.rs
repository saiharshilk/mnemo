use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::theme;
use super::welcome::centered_rect;

pub fn draw(f: &mut Frame, user_code: &str, verification_uri: &str) {
    let area = f.area();
    let bounds = centered_rect(70, 8, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(bounds);
    f.render_widget(block, bounds);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(1), // "visit this url"
            Constraint::Length(1), // url (may wrap)
            Constraint::Length(1), // blank
            Constraint::Length(1), // code label
            Constraint::Length(1), // spaced code
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "github device login",
            theme::title().fg(theme::ACCENT),
        ))),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "visit this url in your browser:",
            theme::dim(),
        ))),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(verification_uri).wrap(Wrap { trim: true }),
        rows[2],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled("your code:", theme::dim()))),
        rows[4],
    );

    // Space out the user_code characters so it reads prominently ("A B C D - 1 2 3 4").
    let spaced: String = user_code
        .chars()
        .flat_map(|c| [c, ' '])
        .collect::<String>()
        .trim_end()
        .to_string();
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            spaced,
            theme::title().fg(theme::ACCENT),
        ))),
        rows[5],
    );

    let hint_y = area.y + area.height.saturating_sub(1);
    let hint = Paragraph::new(
        "press enter to open github in your browser, or visit the url above manually  ·  esc cancel",
    )
    .style(theme::hint());
    f.render_widget(hint, Rect::new(area.x, hint_y, area.width, 1));
}
