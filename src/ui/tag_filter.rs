use crate::db::{CardWithReview, Deck};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::theme;

pub fn draw(
    f: &mut Frame,
    deck: &Deck,
    tags: &[String],
    selected: usize,
    toggled: &[bool],
    status_hint: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(format!(" {} — Tag Filter ", deck.name))
        .title_style(theme::title());

    if tags.is_empty() {
        let empty = Paragraph::new("No tags in this deck")
            .block(block)
            .style(theme::dim());
        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let mark = if toggled.get(i).copied().unwrap_or(false) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let line = Line::from(vec![
                    Span::raw(format!("{mark} ")),
                    Span::raw(tag.clone()),
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

pub fn tag_filter_hint() -> String {
    "↑↓/jk select  Space toggle  r review filtered  Esc back  q quit".to_string()
}
