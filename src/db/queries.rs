use super::models::*;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

fn parse_ts(s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn row_to_deck(row: &rusqlite::Row<'_>) -> rusqlite::Result<Deck> {
    Ok(Deck {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: parse_ts(&row.get::<_, String>(3)?)?,
    })
}

fn row_to_review_state(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<ReviewState> {
    Ok(ReviewState {
        card_id: row.get(offset)?,
        stability: row.get(offset + 1)?,
        difficulty: row.get(offset + 2)?,
        due_date: parse_ts(&row.get::<_, String>(offset + 3)?)?,
        last_review: row
            .get::<_, Option<String>>(offset + 4)?
            .map(|s| parse_ts(&s))
            .transpose()?,
        reps: row.get(offset + 5)?,
        lapses: row.get(offset + 6)?,
        state: CardState::from_str(&row.get::<_, String>(offset + 7)?),
    })
}

fn row_to_card(row: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get(0)?,
        deck_id: row.get(1)?,
        front: row.get(2)?,
        back: row.get(3)?,
        tags: row.get(4)?,
        note_type: row.get(5)?,
        created_at: parse_ts(&row.get::<_, String>(6)?)?,
    })
}

pub fn list_decks(conn: &Connection, now: DateTime<Utc>) -> Result<Vec<DeckSummary>> {
    let now_str = now.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT d.id, d.name, d.description, d.created_at,
                (SELECT COUNT(*) FROM cards c
                 LEFT JOIN review_state rs ON c.id = rs.card_id
                 WHERE c.deck_id = d.id
                   AND (rs.card_id IS NULL OR rs.due_date <= ?1)) AS due_count,
                (SELECT COUNT(*) FROM cards c WHERE c.deck_id = d.id) AS card_count
         FROM decks d
         ORDER BY d.name COLLATE NOCASE",
    )?;
    let decks = stmt
        .query_map(params![now_str], |row| {
            Ok(DeckSummary {
                deck: Deck {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: parse_ts(&row.get::<_, String>(3)?)?,
                },
                due_count: row.get(4)?,
                card_count: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(decks)
}

pub fn get_deck(conn: &Connection, deck_id: i64) -> Result<Option<Deck>> {
    conn.query_row(
        "SELECT id, name, description, created_at FROM decks WHERE id = ?1",
        params![deck_id],
        row_to_deck,
    )
    .optional()
    .context("failed to fetch deck")
}

pub fn create_deck(conn: &Connection, name: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO decks (name, created_at) VALUES (?1, ?2)",
        params![name, now_iso()],
    )
    .context("failed to create deck")?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_deck(conn: &Connection, deck_id: i64, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE decks SET name = ?1 WHERE id = ?2",
        params![name, deck_id],
    )
    .context("failed to rename deck")?;
    Ok(())
}

pub fn delete_deck(conn: &Connection, deck_id: i64) -> Result<()> {
    conn.execute("DELETE FROM decks WHERE id = ?1", params![deck_id])
        .context("failed to delete deck")?;
    Ok(())
}

pub fn list_cards(conn: &Connection, deck_id: i64) -> Result<Vec<CardWithReview>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.deck_id, c.front, c.back, c.tags, c.note_type, c.created_at,
                rs.card_id, rs.stability, rs.difficulty, rs.due_date, rs.last_review,
                rs.reps, rs.lapses, rs.state
         FROM cards c
         LEFT JOIN review_state rs ON c.id = rs.card_id
         WHERE c.deck_id = ?1
         ORDER BY c.id",
    )?;
    let cards = stmt
        .query_map(params![deck_id], |row| {
            let card = row_to_card(row)?;
            let review = row.get::<_, Option<i64>>(7)?.map(|_| row_to_review_state(row, 7)).transpose()?;
            Ok(CardWithReview { card, review })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn get_card(conn: &Connection, card_id: i64) -> Result<Option<Card>> {
    conn.query_row(
        "SELECT id, deck_id, front, back, tags, note_type, created_at FROM cards WHERE id = ?1",
        params![card_id],
        row_to_card,
    )
    .optional()
    .context("failed to fetch card")
}

pub fn create_card(
    conn: &Connection,
    deck_id: i64,
    front: &str,
    back: &str,
    tags: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO cards (deck_id, front, back, tags, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![deck_id, front, back, tags, now_iso()],
    )
    .context("failed to create card")?;
    Ok(conn.last_insert_rowid())
}

pub fn update_card(
    conn: &Connection,
    card_id: i64,
    front: &str,
    back: &str,
    tags: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE cards SET front = ?1, back = ?2, tags = ?3 WHERE id = ?4",
        params![front, back, tags, card_id],
    )
    .context("failed to update card")?;
    Ok(())
}

pub fn delete_card(conn: &Connection, card_id: i64) -> Result<()> {
    conn.execute("DELETE FROM cards WHERE id = ?1", params![card_id])
        .context("failed to delete card")?;
    Ok(())
}

pub fn get_due_cards(conn: &Connection, deck_id: i64, now: DateTime<Utc>) -> Result<Vec<CardWithReview>> {
    let now_str = now.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT c.id, c.deck_id, c.front, c.back, c.tags, c.note_type, c.created_at,
                rs.card_id, rs.stability, rs.difficulty, rs.due_date, rs.last_review,
                rs.reps, rs.lapses, rs.state
         FROM cards c
         LEFT JOIN review_state rs ON c.id = rs.card_id
         WHERE c.deck_id = ?1
           AND (rs.card_id IS NULL OR rs.due_date <= ?2)
         ORDER BY COALESCE(rs.due_date, c.created_at), c.id",
    )?;
    let cards = stmt
        .query_map(params![deck_id, now_str], |row| {
            let card = row_to_card(row)?;
            let review = row.get::<_, Option<i64>>(7)?.map(|_| row_to_review_state(row, 7)).transpose()?;
            Ok(CardWithReview { card, review })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cards)
}

pub fn upsert_review_state(conn: &Connection, state: &ReviewState) -> Result<()> {
    conn.execute(
        "INSERT INTO review_state (card_id, stability, difficulty, due_date, last_review, reps, lapses, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(card_id) DO UPDATE SET
           stability = excluded.stability,
           difficulty = excluded.difficulty,
           due_date = excluded.due_date,
           last_review = excluded.last_review,
           reps = excluded.reps,
           lapses = excluded.lapses,
           state = excluded.state",
        params![
            state.card_id,
            state.stability,
            state.difficulty,
            state.due_date.to_rfc3339(),
            state.last_review.map(|t| t.to_rfc3339()),
            state.reps,
            state.lapses,
            state.state.as_str(),
        ],
    )
    .context("failed to upsert review state")?;
    Ok(())
}

pub fn insert_review_log(
    conn: &Connection,
    card_id: i64,
    rating: i32,
    reviewed_at: DateTime<Utc>,
    elapsed_days: f64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO review_log (card_id, rating, reviewed_at, elapsed_days) VALUES (?1, ?2, ?3, ?4)",
        params![card_id, rating, reviewed_at.to_rfc3339(), elapsed_days],
    )
    .context("failed to insert review log")?;
    Ok(())
}
