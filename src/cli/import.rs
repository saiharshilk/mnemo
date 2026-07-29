use crate::db::{self, open_connection};
use crate::text::detect_note_type;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;

pub fn run_import(path: &Path) -> Result<()> {
    let conn = open_connection()?;
    let decks = db::list_deck_names(&conn)?;
    let deck_id = prompt_deck(&conn, &decks)?;

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("failed to open CSV: {}", path.display()))?;

    let headers = rdr.headers()?.clone();
    let front_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("front"));
    let back_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("back"));
    let tags_idx = headers.iter().position(|h| h.eq_ignore_ascii_case("tags"));

    let (front_idx, back_idx) = match (front_idx, back_idx) {
        (Some(f), Some(b)) => (f, b),
        _ => anyhow::bail!("CSV must have front and back columns"),
    };

    let mut count = 0usize;
    for result in rdr.records() {
        let record = result.context("failed to read CSV row")?;
        let front = record.get(front_idx).unwrap_or("").trim();
        let back = record.get(back_idx).unwrap_or("").trim();
        if front.is_empty() && back.is_empty() {
            continue;
        }
        let tags = tags_idx
            .and_then(|i| record.get(i))
            .map(str::trim)
            .filter(|t| !t.is_empty());
        let note_type = detect_note_type(front);
        db::create_card_with_type(&conn, deck_id, front, back, tags, note_type)?;
        count += 1;
    }

    println!("Imported {count} card(s).");
    Ok(())
}

fn prompt_deck(conn: &rusqlite::Connection, decks: &[(i64, String)]) -> Result<i64> {
    println!("Select a deck to import into:\n");
    for (i, (_, name)) in decks.iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }
    println!("  {}. [Create new deck]", decks.len() + 1);
    print!("\nChoice: ");
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let choice: usize = line
        .trim()
        .parse()
        .context("enter a number from the list")?;

    if choice == 0 || choice > decks.len() + 1 {
        anyhow::bail!("invalid choice");
    }

    if choice <= decks.len() {
        Ok(decks[choice - 1].0)
    } else {
        print!("New deck name: ");
        io::stdout().flush()?;
        line.clear();
        io::stdin().read_line(&mut line)?;
        let name = line.trim();
        if name.is_empty() {
            anyhow::bail!("deck name cannot be empty");
        }
        db::create_deck(conn, name)
    }
}
