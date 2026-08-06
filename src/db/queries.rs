use super::models::*;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};

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

pub fn list_deck_names(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare("SELECT id, name FROM decks ORDER BY name COLLATE NOCASE")?;
    let names = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(names)
}

pub fn deck_name_exists(conn: &Connection, name: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM decks WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
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

pub fn search_cards(conn: &Connection, query: &str) -> Result<Vec<(Card, String)>> {
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT c.id, c.deck_id, c.front, c.back, c.tags, c.note_type, c.created_at,
                d.name
         FROM cards c
         JOIN decks d ON d.id = c.deck_id
         WHERE LOWER(c.front) LIKE ?1 OR LOWER(c.back) LIKE ?1
         ORDER BY d.name COLLATE NOCASE, c.id",
    )?;
    let cards = stmt
        .query_map(params![pattern], |row| Ok((row_to_card(row)?, row.get(7)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cards)
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
            let review = row
                .get::<_, Option<i64>>(7)?
                .map(|_| row_to_review_state(row, 7))
                .transpose()?;
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

pub fn create_card_with_type(
    conn: &Connection,
    deck_id: i64,
    front: &str,
    back: &str,
    tags: Option<&str>,
    note_type: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO cards (deck_id, front, back, tags, note_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![deck_id, front, back, tags, note_type, now_iso()],
    )
    .context("failed to create card")?;
    Ok(conn.last_insert_rowid())
}

pub fn create_card(
    conn: &Connection,
    deck_id: i64,
    front: &str,
    back: &str,
    tags: Option<&str>,
) -> Result<i64> {
    create_card_with_type(conn, deck_id, front, back, tags, "basic")
}

pub fn import_cards(conn: &mut Connection, deck_id: i64, cards: &[ImportedCard]) -> Result<usize> {
    let tx = conn.transaction().context("failed to begin card import")?;
    let now = now_iso();
    let mut inserted = 0;
    {
        for card in cards {
            tx.execute(
            "INSERT INTO cards (deck_id, front, back, tags, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                deck_id,
                card.front,
                card.back,
                card.tags,
                now,
            ],
        )?;
            let card_id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO review_state (card_id, due_date, state) VALUES (?1, ?2, 'new')",
                params![card_id, now],
            )?;
            inserted += 1;
        }
    }
    tx.commit().context("failed to commit card import")?;
    Ok(inserted)
}

pub fn create_deck_and_import_cards(
    conn: &mut Connection,
    name: &str,
    cards: &[ImportedCard],
) -> Result<(i64, usize)> {
    let tx = conn.transaction().context("failed to begin deck import")?;
    let now = now_iso();
    tx.execute(
        "INSERT INTO decks (name, created_at) VALUES (?1, ?2)",
        params![name, now],
    )?;
    let deck_id = tx.last_insert_rowid();
    for card in cards {
        tx.execute(
            "INSERT INTO cards (deck_id, front, back, tags, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![deck_id, card.front, card.back, card.tags, now],
        )?;
        let card_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO review_state (card_id, due_date, state) VALUES (?1, ?2, 'new')",
            params![card_id, now],
        )?;
    }
    tx.commit().context("failed to commit deck import")?;
    Ok((deck_id, cards.len()))
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

pub fn get_due_cards(
    conn: &Connection,
    deck_id: i64,
    now: DateTime<Utc>,
) -> Result<Vec<CardWithReview>> {
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
            let review = row
                .get::<_, Option<i64>>(7)?
                .map(|_| row_to_review_state(row, 7))
                .transpose()?;
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

/// Retention rate over the last 30 days. Returns None when no reviews exist
/// in that window.
pub fn retention_rate_30d(conn: &Connection) -> Result<Option<f64>> {
    let cutoff = (Utc::now() - Duration::days(30)).to_rfc3339();
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM review_log WHERE reviewed_at >= ?1",
        params![cutoff],
        |row| row.get(0),
    )?;
    if total == 0 {
        return Ok(None);
    }
    let success: i64 = conn.query_row(
        "SELECT COUNT(*) FROM review_log WHERE reviewed_at >= ?1 AND rating >= 2",
        params![cutoff],
        |row| row.get(0),
    )?;
    Ok(Some(success as f64 / total as f64 * 100.0))
}

/// Count of reviews per day for the last 90 days.
pub fn review_heatmap_90d(conn: &Connection) -> Result<Vec<(NaiveDate, i64)>> {
    let cutoff = (Utc::now() - Duration::days(90)).to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT DATE(reviewed_at) AS day, COUNT(*) AS cnt
         FROM review_log
         WHERE reviewed_at >= ?1
         GROUP BY day
         ORDER BY day",
    )?;
    let rows = stmt.query_map(params![cutoff], |row| {
        let day_str: String = row.get(0)?;
        let day = NaiveDate::parse_from_str(&day_str, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok((day, row.get(1)?))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

/// Count of cards due per day for the next 14 days (including today).
pub fn forecast_14d(conn: &Connection) -> Result<Vec<(NaiveDate, i64)>> {
    let start = Utc::now().date_naive();
    let end = start + Duration::days(13);
    let start_dt = start
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| start.and_hms_opt(0, 0, 0).unwrap())
        .and_utc();
    let end_dt = end
        .and_hms_opt(23, 59, 59)
        .unwrap_or_else(|| end.and_hms_opt(0, 0, 0).unwrap())
        .and_utc();
    let mut stmt = conn.prepare(
        "SELECT DATE(due_date) AS day, COUNT(*) AS cnt
         FROM review_state
         WHERE due_date >= ?1 AND due_date <= ?2
         GROUP BY day
         ORDER BY day",
    )?;
    let rows = stmt.query_map(params![start_dt.to_rfc3339(), end_dt.to_rfc3339()], |row| {
        let day_str: String = row.get(0)?;
        let day = NaiveDate::parse_from_str(&day_str, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok((day, row.get(1)?))
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::MIGRATION_SQL).unwrap();
        conn
    }

    fn iso(days_ago: i64) -> DateTime<Utc> {
        Utc::now() - Duration::days(days_ago)
    }

    #[test]
    fn test_search_cards_matches_front_and_back_case_insensitively() {
        let conn = in_memory_conn();
        let spanish = create_deck(&conn, "Spanish").unwrap();
        let science = create_deck(&conn, "Science").unwrap();
        create_card(&conn, spanish, "Ab-", "away", None).unwrap();
        create_card(&conn, science, "gravity", "fuerza", None).unwrap();
        create_card(&conn, science, "unrelated", "other", None).unwrap();

        let front_matches = search_cards(&conn, "AB-").unwrap();
        assert_eq!(front_matches.len(), 1);
        assert_eq!(front_matches[0].0.front, "Ab-");
        assert_eq!(front_matches[0].1, "Spanish");

        let back_matches = search_cards(&conn, "FUERZA").unwrap();
        assert_eq!(back_matches.len(), 1);
        assert_eq!(back_matches[0].0.front, "gravity");
    }

    #[test]
    fn test_search_cards_empty_query_returns_all_cards() {
        let conn = in_memory_conn();
        let deck_id = create_deck(&conn, "Deck").unwrap();
        create_card(&conn, deck_id, "one", "first", None).unwrap();
        create_card(&conn, deck_id, "two", "second", None).unwrap();

        assert_eq!(search_cards(&conn, "").unwrap().len(), 2);
    }

    #[test]
    fn test_retention_rate_30d_populated() {
        let conn = in_memory_conn();
        let deck_id = create_deck(&conn, "test").unwrap();
        let card_id = create_card(&conn, deck_id, "q", "a", None).unwrap();

        insert_review_log(&conn, card_id, 3, iso(1), 0.0).unwrap();
        insert_review_log(&conn, card_id, 1, iso(2), 0.0).unwrap();
        insert_review_log(&conn, card_id, 2, iso(3), 0.0).unwrap();

        let rate = retention_rate_30d(&conn).unwrap();
        assert!((rate.unwrap() - 66.6667).abs() < 0.001);
    }

    #[test]
    fn test_retention_rate_30d_empty() {
        let conn = in_memory_conn();
        assert!(retention_rate_30d(&conn).unwrap().is_none());
    }

    #[test]
    fn test_review_heatmap_90d_populated() {
        let conn = in_memory_conn();
        let deck_id = create_deck(&conn, "test").unwrap();
        let card_id = create_card(&conn, deck_id, "q", "a", None).unwrap();

        insert_review_log(&conn, card_id, 3, iso(1), 0.0).unwrap();
        insert_review_log(&conn, card_id, 3, iso(1), 0.0).unwrap();
        insert_review_log(&conn, card_id, 1, iso(5), 0.0).unwrap();

        let heatmap = review_heatmap_90d(&conn).unwrap();
        assert_eq!(heatmap.len(), 2);
        assert_eq!(heatmap.iter().map(|(_, c)| c).sum::<i64>(), 3);
    }

    #[test]
    fn test_review_heatmap_90d_empty() {
        let conn = in_memory_conn();
        assert!(review_heatmap_90d(&conn).unwrap().is_empty());
    }

    #[test]
    fn test_forecast_14d_populated() {
        let conn = in_memory_conn();
        let deck_id = create_deck(&conn, "test").unwrap();
        let card_today = create_card(&conn, deck_id, "q1", "a1", None).unwrap();
        let card_tomorrow = create_card(&conn, deck_id, "q2", "a2", None).unwrap();

        // Create a review_state entry due today and tomorrow.
        use crate::db::models::CardState;
        let state_today = ReviewState {
            card_id: card_today,
            stability: 1.0,
            difficulty: 1.0,
            due_date: Utc::now(),
            last_review: None,
            reps: 1,
            lapses: 0,
            state: CardState::New,
        };
        let state_tomorrow = ReviewState {
            card_id: card_tomorrow,
            stability: 1.0,
            difficulty: 1.0,
            due_date: Utc::now() + Duration::days(1),
            last_review: None,
            reps: 1,
            lapses: 0,
            state: CardState::New,
        };
        upsert_review_state(&conn, &state_today).unwrap();
        upsert_review_state(&conn, &state_tomorrow).unwrap();

        let forecast = forecast_14d(&conn).unwrap();
        assert_eq!(forecast.len(), 2);
        let total: i64 = forecast.iter().map(|(_, c)| c).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn test_forecast_14d_empty() {
        let conn = in_memory_conn();
        assert!(forecast_14d(&conn).unwrap().is_empty());
    }
}
