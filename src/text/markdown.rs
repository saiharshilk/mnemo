use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

pub fn markdown_spans(text: &str) -> Vec<Span<'static>> {
    markdown_spans_with_style(text, Style::default())
}

pub fn markdown_spans_with_style(text: &str, base: Style) -> Vec<Span<'static>> {
    if text.is_empty() {
        return vec![];
    }
    let mut spans = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        if let Some(pos) = rest.find("**") {
            if pos > 0 {
                spans.extend(plain_spans(&rest[..pos], base));
            }
            rest = &rest[pos + 2..];
            if let Some(end) = rest.find("**") {
                let inner = &rest[..end];
                spans.extend(markdown_spans_with_style(
                    inner,
                    base.add_modifier(Modifier::BOLD),
                ));
                rest = &rest[end + 2..];
            } else {
                spans.push(Span::styled(format!("**{rest}"), base));
                break;
            }
        } else if let Some(pos) = rest.find('`') {
            if pos > 0 {
                spans.extend(plain_spans(&rest[..pos], base));
            }
            rest = &rest[pos + 1..];
            if let Some(end) = rest.find('`') {
                spans.push(Span::styled(
                    rest[..end].to_string(),
                    base.add_modifier(Modifier::UNDERLINED),
                ));
                rest = &rest[end + 1..];
            } else {
                spans.push(Span::styled(format!("`{rest}"), base));
                break;
            }
        } else if let Some(pos) = rest.find('*') {
            if pos > 0 {
                spans.extend(plain_spans(&rest[..pos], base));
            }
            rest = &rest[pos + 1..];
            if let Some(end) = rest.find('*') {
                let inner = &rest[..end];
                spans.extend(markdown_spans_with_style(
                    inner,
                    base.add_modifier(Modifier::ITALIC),
                ));
                rest = &rest[end + 1..];
            } else {
                spans.push(Span::styled(format!("*{rest}"), base));
                break;
            }
        } else {
            spans.extend(plain_spans(rest, base));
            break;
        }
    }

    spans
}

fn plain_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    if base == Style::default() {
        vec![Span::raw(text.to_string())]
    } else {
        vec![Span::styled(text.to_string(), base)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn parses_bold_and_italic() {
        let spans = markdown_spans("**bold** and *italic*");
        assert!(spans.iter().any(|s| s.style.add_modifier.contains(Modifier::BOLD)));
        assert!(spans.iter().any(|s| s.style.add_modifier.contains(Modifier::ITALIC)));
    }

    #[test]
    fn parses_inline_code() {
        let spans = markdown_spans("use `Vec` here");
        assert!(spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::UNDERLINED)));
    }
}
