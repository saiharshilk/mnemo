use crate::db::{self, Card};
use anyhow::{Context, Result};
use csv::{QuoteStyle, WriterBuilder};
use rusqlite::Connection;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

const BASIC_DIRECTIVES: &[&str] = &[
    "#separator:Comma",
    "#html:true",
    "#notetype:Basic",
    "#columns:Front,Back,Tags",
    "#tags column:3",
];
const CLOZE_DIRECTIVES: &[&str] = &[
    "#separator:Comma",
    "#html:true",
    "#notetype:Cloze",
    "#columns:Text,Back Extra,Tags",
    "#tags column:3",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnkiExportPreview {
    pub basic_count: usize,
    pub cloze_count: usize,
    pub skipped_rows: usize,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnkiExportPlan {
    preview: AnkiExportPreview,
    basic_rows: Vec<[String; 3]>,
    cloze_rows: Vec<[String; 3]>,
}

pub fn preview_deck(
    conn: &Connection,
    deck_id: i64,
    deck_name: &str,
    output: &Path,
) -> Result<AnkiExportPreview> {
    let cards = db::list_cards(conn, deck_id)?;
    let cards: Vec<Card> = cards.into_iter().map(|entry| entry.card).collect();
    Ok(build_plan(deck_name, &cards, output).preview)
}

pub fn export_deck(
    conn: &Connection,
    deck_id: i64,
    deck_name: &str,
    output: &Path,
) -> Result<AnkiExportPreview> {
    let cards = db::list_cards(conn, deck_id)?;
    let cards: Vec<Card> = cards.into_iter().map(|entry| entry.card).collect();
    let plan = build_plan(deck_name, &cards, output);
    write_plan(&plan)?;
    Ok(plan.preview)
}

pub fn export_cards(deck_name: &str, cards: &[Card], output: &Path) -> Result<AnkiExportPreview> {
    let plan = build_plan(deck_name, cards, output);
    write_plan(&plan)?;
    Ok(plan.preview)
}

fn build_plan(deck_name: &str, cards: &[Card], output: &Path) -> AnkiExportPlan {
    let mut basic_rows = Vec::new();
    let mut cloze_rows = Vec::new();
    let mut skipped_rows = 0;

    for card in cards {
        let is_cloze = card.note_type.eq_ignore_ascii_case("cloze") || card.front.contains("{{c");
        let tags = normalize_tags(card.tags.as_deref().unwrap_or(""));

        if is_cloze {
            match convert_cloze_to_html(&card.front) {
                Ok(text) if !text.is_empty() => {
                    cloze_rows.push([text, String::new(), tags]);
                }
                _ => skipped_rows += 1,
            }
        } else if card.front.trim().is_empty() || card.back.trim().is_empty() {
            skipped_rows += 1;
        } else {
            basic_rows.push([
                markdown_to_html(&card.front),
                markdown_to_html(&card.back),
                tags,
            ]);
        }
    }

    let paths = output_paths(
        deck_name,
        output,
        !basic_rows.is_empty(),
        !cloze_rows.is_empty(),
    );
    AnkiExportPlan {
        preview: AnkiExportPreview {
            basic_count: basic_rows.len(),
            cloze_count: cloze_rows.len(),
            skipped_rows,
            paths,
        },
        basic_rows,
        cloze_rows,
    }
}

fn write_plan(plan: &AnkiExportPlan) -> Result<()> {
    let mut path_index = 0;
    if !plan.basic_rows.is_empty() {
        write_anki_file(
            &plan.preview.paths[path_index],
            BASIC_DIRECTIVES,
            &plan.basic_rows,
        )?;
        path_index += 1;
    }
    if !plan.cloze_rows.is_empty() {
        write_anki_file(
            &plan.preview.paths[path_index],
            CLOZE_DIRECTIVES,
            &plan.cloze_rows,
        )?;
    }
    Ok(())
}

fn write_anki_file(path: &Path, directives: &[&str], rows: &[[String; 3]]) -> Result<()> {
    let file = File::create(path)
        .with_context(|| format!("failed to create Anki CSV: {}", path.display()))?;
    let mut file = BufWriter::new(file);
    for directive in directives {
        writeln!(file, "{directive}")?;
    }

    let mut writer = WriterBuilder::new()
        .delimiter(b',')
        .has_headers(false)
        .quote_style(QuoteStyle::Necessary)
        .from_writer(file);
    for row in rows {
        writer.write_record(row)?;
    }
    writer.flush()?;
    Ok(())
}

fn output_paths(deck_name: &str, output: &Path, has_basic: bool, has_cloze: bool) -> Vec<PathBuf> {
    let base = if output.as_os_str().is_empty() {
        PathBuf::from(deck_name)
    } else if output.is_dir() {
        output.join(deck_name)
    } else {
        output.to_path_buf()
    };
    let base = if base.extension().is_some_and(|ext| ext == "csv") {
        base.with_extension("")
    } else {
        base
    };

    let mut paths = Vec::new();
    if has_basic {
        paths.push(with_suffix(&base, "_basic.csv"));
    }
    if has_cloze {
        paths.push(with_suffix(&base, "_cloze.csv"));
    }
    paths
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut path = base.to_path_buf();
    let file_name = path
        .file_name()
        .map(|name| format!("{}{}", name.to_string_lossy(), suffix))
        .unwrap_or_else(|| suffix.trim_start_matches('_').to_string());
    path.set_file_name(file_name);
    path
}

pub fn normalize_tags(tags: &str) -> String {
    tags.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.split_whitespace().collect::<Vec<_>>().join("-"))
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn markdown_to_html(text: &str) -> String {
    render_markdown(text, 0).0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Marker {
    Bold,
    Italic,
    Code,
}

impl Marker {
    fn delimiter(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "*",
            Self::Code => "`",
        }
    }

    fn open_tag(self) -> &'static str {
        match self {
            Self::Bold => "<b>",
            Self::Italic => "<i>",
            Self::Code => "<code>",
        }
    }

    fn close_tag(self) -> &'static str {
        match self {
            Self::Bold => "</b>",
            Self::Italic => "</i>",
            Self::Code => "</code>",
        }
    }
}

fn render_markdown(text: &str, start: usize) -> (String, usize) {
    let (html, position, _) = render_markdown_until(text, start, None);
    (html, position)
}

fn render_markdown_until(
    text: &str,
    mut position: usize,
    closing: Option<Marker>,
) -> (String, usize, bool) {
    let mut output = String::new();
    while position < text.len() {
        if let Some(marker) = closing {
            if text[position..].starts_with(marker.delimiter()) {
                return (output, position + marker.delimiter().len(), true);
            }
        }

        let marker = if text[position..].starts_with("**") {
            Some(Marker::Bold)
        } else if text[position..].starts_with('`') {
            Some(Marker::Code)
        } else if text[position..].starts_with('*') {
            Some(Marker::Italic)
        } else {
            None
        };

        let Some(marker) = marker else {
            let character = text[position..].chars().next().unwrap();
            append_escaped(&mut output, character);
            position += character.len_utf8();
            continue;
        };

        let delimiter = marker.delimiter();
        let (inner, end, closed) =
            render_markdown_until(text, position + delimiter.len(), Some(marker));
        if closed {
            output.push_str(marker.open_tag());
            output.push_str(&inner);
            output.push_str(marker.close_tag());
            position = end;
        } else {
            output.push_str(&escape_text(delimiter));
            output.push_str(&inner);
            position = end;
        }
    }
    (output, position, false)
}

fn append_escaped(output: &mut String, character: char) {
    match character {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '>' => output.push_str("&gt;"),
        '\n' => output.push_str("<br>"),
        '\r' => {}
        _ => output.push(character),
    }
}

fn escape_text(text: &str) -> String {
    let mut output = String::new();
    for character in text.chars() {
        append_escaped(&mut output, character);
    }
    output
}

fn convert_cloze_to_html(text: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut position = 0;
    let mut next_number = 1;
    let mut found = false;

    while let Some(relative_start) = text[position..].find("{{") {
        let start = position + relative_start;
        output.push_str(&markdown_to_html(&text[position..start]));
        let Some(relative_end) = text[start + 2..].find("}}") else {
            return Err("unterminated cloze".to_string());
        };
        let end = start + 2 + relative_end;
        let token = &text[start + 2..end];
        if !token.starts_with('c') {
            output.push_str(&markdown_to_html(&text[start..end + 2]));
            position = end + 2;
            continue;
        }

        let Some(separator) = token.find("::") else {
            return Err("cloze is missing its separator".to_string());
        };
        let number = &token[1..separator];
        if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
            return Err("invalid cloze number".to_string());
        }
        let content = &token[separator + 2..];
        let (answer, hint) = match content.split_once("::") {
            Some((answer, hint)) => (answer, Some(hint)),
            None => (content, None),
        };
        if answer.is_empty() {
            return Err("cloze answer is empty".to_string());
        }

        output.push_str("{{c");
        output.push_str(&next_number.to_string());
        output.push_str("::");
        output.push_str(&markdown_to_html(answer));
        if let Some(hint) = hint {
            output.push_str("::");
            output.push_str(&markdown_to_html(hint));
        }
        output.push_str("}}");
        next_number += 1;
        found = true;
        position = end + 2;
    }

    output.push_str(&markdown_to_html(&text[position..]));
    if !found {
        return Err("not a cloze card".to_string());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn card(front: &str, back: &str, tags: Option<&str>, note_type: &str) -> Card {
        Card {
            id: 1,
            deck_id: 1,
            front: front.to_string(),
            back: back.to_string(),
            tags: tags.map(str::to_string),
            note_type: note_type.to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn markdown_escapes_html_and_converts_nested_formatting() {
        assert_eq!(
            markdown_to_html("**bold *italic*** <tag> & `code`\nnext"),
            "<b>bold <i>italic</i></b> &lt;tag&gt; &amp; <code>code</code><br>next"
        );
    }

    #[test]
    fn cloze_conversion_renumbers_multiple_blanks_and_preserves_hints() {
        assert_eq!(
            convert_cloze_to_html("**Learn** {{c7::one}} and {{c3::two::hint}}"),
            Ok("<b>Learn</b> {{c1::one}} and {{c2::two::hint}}".to_string())
        );
    }

    #[test]
    fn tags_become_space_separated_hyphenated_tags() {
        assert_eq!(
            normalize_tags("biology, spaced repetition,hard cards,,"),
            "biology spaced-repetition hard-cards"
        );
    }

    #[test]
    fn cloze_export_writes_cloze_directives_and_blank_back_extra() {
        let directory = std::env::temp_dir().join(format!(
            "mnemo-anki-cloze-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let result = export_cards(
            "Deck",
            &[card(
                "See {{c9::**answer**::hint}}.",
                "ignored back",
                Some("cloze tag"),
                "cloze",
            )],
            &directory.join("study.csv"),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&result.paths[0]).unwrap(),
            "#separator:Comma\n#html:true\n#notetype:Cloze\n#columns:Text,Back Extra,Tags\n#tags column:3\nSee {{c1::<b>answer</b>::hint}}.,,cloze-tag\n"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn end_to_end_export_writes_anki_directives_and_csv() {
        let directory = std::env::temp_dir().join(format!(
            "mnemo-anki-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let output = directory.join("study.csv");
        let cards = vec![card(
            "**Front**, <literal>",
            "Back, with `code`",
            Some("one,two words"),
            "basic",
        )];

        let result = export_cards("Deck", &cards, &output).unwrap();
        assert_eq!(result.basic_count, 1);
        assert_eq!(result.cloze_count, 0);
        assert_eq!(
            std::fs::read_to_string(result.paths[0].clone()).unwrap(),
            "#separator:Comma\n#html:true\n#notetype:Basic\n#columns:Front,Back,Tags\n#tags column:3\n\"<b>Front</b>, &lt;literal&gt;\",\"Back, with <code>code</code>\",one two-words\n"
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
