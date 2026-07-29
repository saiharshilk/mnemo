use super::markdown::markdown_spans;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

const CLOZE_HIGHLIGHT: Color = Color::Rgb(180, 140, 80);

/// Returns true if text contains Anki-style cloze deletion syntax.
pub fn is_cloze(text: &str) -> bool {
    text.as_bytes()
        .windows(4)
        .any(|w| w == b"{{c" && text.contains("::"))
}

pub fn detect_note_type(front: &str) -> &'static str {
    if is_cloze(front) {
        "cloze"
    } else {
        "basic"
    }
}

enum Segment {
    Text(String),
    Cloze(String),
}

fn parse_segments(text: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut rest = text;

    while let Some(start) = rest.find("{{c") {
        if start > 0 {
            segments.push(Segment::Text(rest[..start].to_string()));
        }
        rest = &rest[start..];
        let Some(colon) = rest.find("::") else {
            segments.push(Segment::Text(rest.to_string()));
            return segments;
        };
        let after_colon = colon + 2;
        let Some(end) = rest[after_colon..].find("}}") else {
            segments.push(Segment::Text(rest.to_string()));
            return segments;
        };
        let content = rest[after_colon..after_colon + end].to_string();
        segments.push(Segment::Cloze(content));
        rest = &rest[after_colon + end + 2..];
    }

    if !rest.is_empty() {
        segments.push(Segment::Text(rest.to_string()));
    }
    segments
}

pub fn hidden_lines(text: &str) -> Vec<Line> {
    let mut spans = Vec::new();
    for seg in parse_segments(text) {
        match seg {
            Segment::Text(t) => spans.extend(markdown_spans(&t)),
            Segment::Cloze(_) => spans.push(Span::raw("[...]")),
        }
    }
    vec![Line::from(spans)]
}

pub fn revealed_lines(text: &str) -> Vec<Line> {
    let mut spans = Vec::new();
    for seg in parse_segments(text) {
        match seg {
            Segment::Text(t) => spans.extend(markdown_spans(&t)),
            Segment::Cloze(content) => {
                spans.extend(markdown_spans_styled(
                    &content,
                    Style::default().fg(CLOZE_HIGHLIGHT),
                ))
            }
        }
    }
    vec![Line::from(spans)]
}

fn markdown_spans_styled(text: &str, base: ratatui::style::Style) -> Vec<Span<'static>> {
    super::markdown::markdown_spans_with_style(text, base)
}
