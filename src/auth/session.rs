use crate::paths::data_dir;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub github_token: String,
    pub github_id: i64,
    pub github_username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

fn path() -> Result<PathBuf> {
    Ok(data_dir()?.join("session.json"))
}

impl Session {
    /// Loads the persisted session, returning `Ok(None)` when no file is present,
    /// OR when the file is malformed, OR when it cannot be read for any I/O reason.
    /// A bad/illegible session.json should never block the app from launching.
    pub fn load() -> Result<Option<Self>> {
        let path = match path() {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(s) => Ok(Some(s)),
                Err(_) => Ok(None),
            },
            Err(_) => Ok(None),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = path()?;
        // data_dir() already creates the directory, but re-create it here to guard
        // against deletion/races between path resolution and the actual write.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create session directory: {}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self).context("failed to serialize session")?;
        fs::write(&path, bytes)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}
