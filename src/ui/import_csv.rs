use crate::app::ImportStep;
use crate::csv_import::CsvPreview;
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
    step: ImportStep,
    input: &str,
    error: Option<&str>,
    preview: Option<&CsvPreview>,
    decks: &[(i64, String)],
    selected: usize,
    deck_name: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(f.area());

    let title = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(format!(" Import CSV — {} ", step.title()))
        .title_style(theme::title());

    match step {
        ImportStep::FilePath | ImportStep::NewDeckName => {
            let prompt = if step == ImportStep::FilePath {
                "csv file path:"
            } else {
                "new deck name:"
            };
            let mut lines = vec![Line::from(vec![
                Span::raw(prompt),
                Span::raw(" "),
                Span::raw(format!("{input}_")),
            ])];
            if let Some(error) = error {
                lines.push(Line::from(error));
            }
            f.render_widget(Paragraph::new(lines).block(title), chunks[0]);
        }
        ImportStep::Preview => {
            let mut lines = vec![Line::from("csv preview")];
            if let Some(preview) = preview {
                lines.push(Line::from(format!(
                    "{} rows found in this file ({} cards ready to import)",
                    preview.total_rows,
                    preview.cards.len()
                )));
                lines.push(Line::from(""));
                for (index, row) in preview.preview_rows.iter().enumerate() {
                    lines.push(Line::from(format!(
                        "{:>2}. {:<32}  {}",
                        index + 1,
                        truncate(&row.front, 32),
                        truncate(&row.back, 32)
                    )));
                }
                if preview.skipped_rows > 0 {
                    lines.push(Line::from(format!(
                        "{} rows will be skipped during import",
                        preview.skipped_rows
                    )));
                }
            }
            f.render_widget(Paragraph::new(lines).block(title), chunks[0]);
        }
        ImportStep::DeckChoice => {
            let existing_label = if decks.is_empty() {
                "existing deck (none available)"
            } else {
                "existing deck"
            };
            let labels = ["create new deck", existing_label];
            let mut items = vec![ListItem::new("add to which deck?")];
            items.extend(labels.into_iter().enumerate().map(|(index, label)| {
                let style = if index == selected {
                    theme::selected()
                } else {
                    Style::default()
                };
                ListItem::new(format!(
                    "{} {}",
                    if index == selected { ">" } else { " " },
                    label
                ))
                .style(style)
            }));
            f.render_widget(List::new(items).block(title), chunks[0]);
        }
        ImportStep::ExistingDeck => {
            let items = decks
                .iter()
                .enumerate()
                .map(|(index, (_, name))| {
                    let style = if index == selected {
                        theme::selected()
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!(
                        "{} {}",
                        if index == selected { ">" } else { " " },
                        name
                    ))
                    .style(style)
                })
                .collect::<Vec<_>>();
            let items = if items.is_empty() {
                vec![ListItem::new(
                    "No existing decks. Esc to choose create new deck.",
                )]
            } else {
                items
            };
            f.render_widget(List::new(items).block(title), chunks[0]);
        }
        ImportStep::Confirm => {
            let card_count = preview.map(|p| p.cards.len()).unwrap_or(0);
            let skipped = preview.map(|p| p.skipped_rows).unwrap_or(0);
            let mut summary = format!(
                "import {} cards into '{}' ?",
                card_count,
                truncate(deck_name, 48)
            );
            if skipped > 0 {
                summary.push_str(&format!("\n{} rows will be skipped", skipped));
            }
            if let Some(error) = error {
                summary.push_str(&format!("\n{}", error));
            }
            f.render_widget(Paragraph::new(summary).block(title), chunks[0]);
        }
    }

    let hint = Paragraph::new(step.hint()).style(theme::hint());
    f.render_widget(hint, chunks[1]);
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
