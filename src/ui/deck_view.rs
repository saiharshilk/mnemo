use crate::db::{CardState, CardWithReview, Deck};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::theme;

pub fn draw(
    f: &mut Frame,
    deck: &Deck,
    cards: &[CardWithReview],
    selected: usize,
    status_hint: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(f.area());

    let title = format!(" {} ", deck.name);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(title)
        .title_style(theme::title());

    if cards.is_empty() {
        let empty = Paragraph::new("No cards yet — press n to add one")
            .block(block)
            .style(theme::dim());
        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = cards
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let state = entry
                    .review
                    .as_ref()
                    .map(|r| r.state)
                    .unwrap_or(CardState::New);
                let glyph = state.glyph();
                let label = state.label();
                let front = truncate(&entry.card.front, 48);
                let line = Line::from(vec![
                    Span::raw(format!(" {} {:>8}  ", glyph, label)),
                    Span::raw(front),
                ]);
                let style = if i == selected {
                    theme::selected()
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, chunks[0]);
    }

    let bar = Paragraph::new(status_hint).style(theme::hint());
    f.render_widget(bar, chunks[1]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

pub fn deck_view_hint(delete_pending: bool) -> String {
    if delete_pending {
        "Press d again to confirm delete  ·  Esc cancel".to_string()
    } else {
        "↑↓/jk select  Enter/e edit  n new  d delete  r review  Esc back  q quit".to_string()
    }
}
