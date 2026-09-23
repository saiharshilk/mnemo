use crate::anki_export::AnkiExportPreview;
use crate::app::AnkiExportStep;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::theme;

pub fn draw(
    f: &mut Frame,
    step: AnkiExportStep,
    input: &str,
    error: Option<&str>,
    preview: Option<&AnkiExportPreview>,
    deck_name: &str,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(f.area());
    let title = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(format!(" Export Anki — {} ", step.title()))
        .title_style(theme::title());

    let mut lines = Vec::new();
    match step {
        AnkiExportStep::FilePath => {
            lines.push(Line::from(vec![
                Span::raw("output path/base (blank uses deck name): "),
                Span::raw(format!("{input}_")),
            ]));
        }
        AnkiExportStep::Preview => {
            lines.push(Line::from("anki export preview"));
            if let Some(preview) = preview {
                lines.push(Line::from(format!(
                    "{} Basic card(s), {} Cloze card(s)",
                    preview.basic_count, preview.cloze_count
                )));
                lines.push(Line::from(format!("deck: {deck_name}")));
                for path in &preview.paths {
                    lines.push(Line::from(format!("file: {}", path.display())));
                }
                if preview.skipped_rows > 0 {
                    lines.push(Line::from(format!(
                        "{} malformed/empty row(s) will be skipped",
                        preview.skipped_rows
                    )));
                }
            }
        }
        AnkiExportStep::Confirm => {
            if let Some(preview) = preview {
                lines.push(Line::from(format!(
                    "export {} Basic and {} Cloze card(s) from '{}' ?",
                    preview.basic_count, preview.cloze_count, deck_name
                )));
                for path in &preview.paths {
                    lines.push(Line::from(format!("file: {}", path.display())));
                }
                if preview.skipped_rows > 0 {
                    lines.push(Line::from(format!(
                        "{} row(s) will be skipped",
                        preview.skipped_rows
                    )));
                }
            }
        }
    }
    if let Some(error) = error {
        lines.push(Line::from(error));
    }

    f.render_widget(Paragraph::new(lines).block(title), chunks[0]);
    f.render_widget(Paragraph::new(step.hint()).style(theme::hint()), chunks[1]);
}
