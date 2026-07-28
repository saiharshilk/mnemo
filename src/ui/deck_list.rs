use crate::db::DeckSummary;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::theme;

pub fn draw(f: &mut Frame, decks: &[DeckSummary], selected: usize, status_hint: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(f.area());

    let title = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(" Decks ")
        .title_style(theme::title());

    if decks.is_empty() {
        let empty = Paragraph::new("No decks yet — press n to create one")
            .block(title)
            .style(theme::dim());
        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = decks
            .iter()
            .enumerate()
            .map(|(i, summary)| {
                let due_badge = if summary.due_count > 0 {
                    Span::styled(
                        format!(" {:>3} due ", summary.due_count),
                        theme::accent(),
                    )
                } else {
                    Span::styled("       ", Style::default())
                };
                let name = format!("{:<24}", summary.deck.name);
                let cards = format!("{:>4} cards", summary.card_count);
                let line = Line::from(vec![
                    Span::raw(name),
                    due_badge,
                    Span::raw("  "),
                    Span::styled(cards, theme::dim()),
                ]);
                let style = if i == selected {
                    theme::selected()
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect();

        let list = List::new(items).block(title);
        f.render_widget(list, chunks[0]);
    }

    draw_status_bar(f, chunks[1], status_hint);
}

fn draw_status_bar(f: &mut Frame, area: Rect, hint: &str) {
    let bar = Paragraph::new(hint).style(theme::hint());
    f.render_widget(bar, area);
}

pub fn deck_list_hint(delete_pending: bool) -> String {
    if delete_pending {
        "Press d again to confirm delete  ·  Esc cancel".to_string()
    } else {
        "↑↓/jk select  Enter open  n new  e rename  d delete  q quit".to_string()
    }
}
