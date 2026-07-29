use anyhow::{Context, Result};
use std::path::PathBuf;

/// Returns `~/.local/share/mnemo` (Linux) / `~/Library/Application Support/mnemo` (macOS)
/// / `%LOCALAPPDATA%\mnemo` (Windows). Creates the directory if missing.
pub fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("could not resolve local data directory")?;
    let dir = base.join("mnemo");
    std::fs::create_dir_all(&dir).with_context(|| {
        format!("failed to create data directory: {}", dir.display())
    })?;
    Ok(dir)
}
