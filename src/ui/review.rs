use crate::db::CardWithReview;
use crate::fsrs::scheduler::{format_interval, preview_intervals};
use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::theme;

pub fn draw(
    f: &mut Frame,
    current: Option<&CardWithReview>,
    flipped: bool,
    queue_remaining: usize,
    message: Option<&str>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(if flipped { 3 } else { 1 }),
            Constraint::Length(1),
        ])
        .split(f.area());

    if let Some(msg) = message {
        let msg_para = Paragraph::new(msg)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .title(" Review ")
                    .title_style(theme::title()),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(msg_para, chunks[1]);
        let hint = Paragraph::new("Esc back  q quit").style(theme::hint());
        f.render_widget(hint, chunks[3]);
        return;
    }

    if current.is_none() {
        let empty = Paragraph::new("All caught up — nothing due")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .title(" Review ")
                    .title_style(theme::title()),
            )
            .alignment(Alignment::Center)
            .style(theme::dim());
        f.render_widget(empty, chunks[1]);
        let hint = Paragraph::new("Esc back  q quit").style(theme::hint());
        f.render_widget(hint, chunks[3]);
        return;
    }

    let entry = current.unwrap();
    let header = Paragraph::new(format!("Review  ·  {queue_remaining} remaining"))
        .style(theme::dim())
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    let content = if flipped {
        &entry.card.back
    } else {
        &entry.card.front
    };
    let side = if flipped { "Back" } else { "Front" };

    let card_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(format!(" {side} "))
        .title_style(theme::title());

    let card = Paragraph::new(content.as_str())
        .block(card_block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(card, chunks[1]);

    if flipped {
        let now = Utc::now();
        let intervals = preview_intervals(entry.review.as_ref(), now);
        let ratings = [
            ("1 again", intervals[0]),
            ("2 hard", intervals[1]),
            ("3 good", intervals[2]),
            ("4 easy", intervals[3]),
        ];
        let spans: Vec<Span> = ratings
            .iter()
            .flat_map(|(label, dur)| {
                [
                    Span::styled(*label, theme::accent()),
                    Span::raw(format!(" ({})  ", format_interval(*dur))),
                ]
            })
            .collect();
        f.render_widget(Paragraph::new(Line::from(spans)), chunks[2]);
        let hint = Paragraph::new("1-4 rate  Esc end session  q quit").style(theme::hint());
        f.render_widget(hint, chunks[3]);
    } else {
        f.render_widget(Paragraph::new(""), chunks[2]);
        let hint = Paragraph::new("Space/Enter flip  Esc end session  q quit").style(theme::hint());
        f.render_widget(hint, chunks[3]);
    }
}
