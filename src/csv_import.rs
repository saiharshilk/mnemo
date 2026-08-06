use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::db::ImportedCard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvPreviewRow {
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone)]
pub struct CsvPreview {
    pub cards: Vec<ImportedCard>,
    pub preview_rows: Vec<CsvPreviewRow>,
    pub total_rows: usize,
    pub skipped_rows: usize,
}

impl CsvPreview {
    pub fn from_path(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("could not read csv file: {}", path.display()))?;
        Self::from_reader(file)
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);
        let headers = csv_reader.headers()?.clone();

        let front_idx = headers
            .iter()
            .position(|header| header.trim().eq_ignore_ascii_case("front"));
        let back_idx = headers
            .iter()
            .position(|header| header.trim().eq_ignore_ascii_case("back"));
        let tags_idx = headers
            .iter()
            .position(|header| header.trim().eq_ignore_ascii_case("tags"));

        let (Some(front_idx), Some(back_idx)) = (front_idx, back_idx) else {
            bail!("csv must have 'front' and 'back' columns");
        };

        let mut cards = Vec::new();
        let mut preview_rows = Vec::new();
        let mut total_rows = 0;
        let mut skipped_rows = 0;

        for record in csv_reader.records() {
            let record = record.context("could not read csv row")?;
            total_rows += 1;

            let front = record.get(front_idx).unwrap_or("").trim();
            let back = record.get(back_idx).unwrap_or("").trim();
            if preview_rows.len() < 3 {
                preview_rows.push(CsvPreviewRow {
                    front: front.to_owned(),
                    back: back.to_owned(),
                });
            }
            if front.is_empty() || back.is_empty() {
                skipped_rows += 1;
                continue;
            }

            let tags = tags_idx
                .and_then(|index| record.get(index))
                .map(str::trim)
                .filter(|tags| !tags.is_empty())
                .map(ToOwned::to_owned);

            cards.push(ImportedCard {
                front: front.to_owned(),
                back: back.to_owned(),
                tags,
            });
        }

        Ok(Self {
            cards,
            preview_rows,
            total_rows,
            skipped_rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn detects_required_headers_case_insensitively() {
        let preview =
            CsvPreview::from_reader(Cursor::new("BACK,Tags,FRONT\nanswer,verbs,question\n"))
                .unwrap();

        assert_eq!(preview.cards.len(), 1);
        assert_eq!(preview.cards[0].front, "question");
        assert_eq!(preview.cards[0].back, "answer");
        assert_eq!(preview.cards[0].tags.as_deref(), Some("verbs"));
    }

    #[test]
    fn rejects_csv_without_front_and_back_headers() {
        let error = CsvPreview::from_reader(Cursor::new("question,answer\na,b\n"))
            .unwrap_err()
            .to_string();

        assert_eq!(error, "csv must have 'front' and 'back' columns");
    }

    #[test]
    fn skips_rows_missing_either_required_value() {
        let preview = CsvPreview::from_reader(Cursor::new(
            "front,back,tags\nquestion,answer,\"one,two\"\n,missing-front,tag\nmissing-back,,tag\nvalid,also valid,tag\n",
        ))
        .unwrap();

        assert_eq!(preview.total_rows, 4);
        assert_eq!(preview.skipped_rows, 2);
        assert_eq!(preview.cards.len(), 2);
        assert_eq!(preview.cards[0].tags.as_deref(), Some("one,two"));
    }
}
