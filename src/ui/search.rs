use crate::db::Card;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use super::theme;

pub fn draw(f: &mut Frame, query: &str, results: &[(Card, String)], selected: usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(" Search ")
        .title_style(theme::title());
    let inner = block.inner(chunks[0]);
    f.render_widget(block, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("search: "),
            Span::raw(format!("{query}_")),
        ])),
        body[0],
    );

    if results.is_empty() {
        let message = if query.is_empty() {
            "No cards yet"
        } else {
            "No cards found"
        };
        f.render_widget(Paragraph::new(message).style(theme::dim()), body[1]);
    } else {
        let items = results
            .iter()
            .map(|(card, deck_name)| {
                ListItem::new(Line::from(format!(
                    "{:<20}  {:<32}  {}",
                    truncate(deck_name, 20),
                    truncate(&card.front, 32),
                    truncate(&card.back, 32)
                )))
            })
            .collect::<Vec<_>>();
        let list = List::new(items)
            .highlight_style(theme::selected())
            .highlight_symbol("> ");
        let mut state = ListState::default();
        state.select(Some(selected));
        f.render_stateful_widget(list, body[1], &mut state);
    }

    f.render_widget(
        Paragraph::new("type to filter  ↑↓ select  Enter edit  Esc back").style(theme::hint()),
        chunks[1],
    );
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
        )
    }
}
