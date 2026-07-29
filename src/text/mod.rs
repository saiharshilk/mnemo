mod cloze;
mod markdown;

pub use cloze::{detect_note_type, hidden_lines, is_cloze, revealed_lines};
pub use markdown::{markdown_spans, markdown_spans_with_style};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cloze_syntax() {
        assert!(is_cloze("The {{c1::mitochondria}} is the powerhouse"));
        assert!(!is_cloze("plain card"));
    }

    #[test]
    fn cloze_hidden_blanks_content() {
        let lines = hidden_lines("The {{c1::mitochondria}} is powerful");
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
            .collect();
        assert!(text.contains("[...]"));
        assert!(!text.contains("mitochondria"));
    }

    #[test]
    fn cloze_revealed_shows_content() {
        let lines = revealed_lines("The {{c1::mitochondria}} is powerful");
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.clone()))
            .collect();
        assert!(text.contains("mitochondria"));
        assert!(!text.contains("[...]"));
    }
}
