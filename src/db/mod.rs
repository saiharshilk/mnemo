mod models;
mod queries;

pub use models::*;
pub use queries::*;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

const MIGRATION_SQL: &str = include_str!("../../migrations/001_init.sql");

pub fn db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("could not resolve local data directory")?;
    Ok(base.join("flashcard-tui").join("data.db"))
}

pub fn open_connection() -> Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create data directory")?;
    }
    let conn = Connection::open(&path).context("failed to open database")?;
    conn.execute_batch(MIGRATION_SQL)
        .context("failed to run migrations")?;
    Ok(conn)
}
