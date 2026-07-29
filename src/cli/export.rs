use crate::db::{self, open_connection};
use anyhow::{Context, Result};
use std::path::Path;

pub fn run_export(deck_name: &str, path: &Path) -> Result<()> {
    let conn = open_connection()?;
    let deck = db::get_deck_by_name(&conn, deck_name)?
        .with_context(|| format!("deck not found: {deck_name}"))?;
    let cards = db::list_cards(&conn, deck.id)?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("failed to create CSV: {}", path.display()))?;

    wtr.write_record(["front", "back", "tags"])?;
    for entry in cards {
        wtr.write_record([
            entry.card.front.as_str(),
            entry.card.back.as_str(),
            entry.card.tags.as_deref().unwrap_or(""),
        ])?;
    }
    wtr.flush()?;

    println!("Exported {} card(s) to {}.", cards.len(), path.display());
    Ok(())
}
