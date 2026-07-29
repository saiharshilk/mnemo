mod models;
mod queries;

pub use models::*;
pub use queries::*;

use crate::paths::data_dir;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

const MIGRATION_SQL: &str = include_str!("../../migrations/001_init.sql");

pub fn db_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("data.db"))
}

pub fn session_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("session.json"))
}

pub fn open_connection() -> Result<Connection> {
    let path = db_path()?;
    let conn = Connection::open(&path).context("failed to open database")?;
    conn.execute_batch(MIGRATION_SQL)
        .context("failed to run migrations")?;
    Ok(conn)
}
