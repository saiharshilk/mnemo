use crate::auth::github::GitHubUser;
use anyhow::{Result, anyhow};

/// Upserts a `users` row in the Supabase PostgREST `users` table.
/// Requires the table to already exist in the Supabase project with columns:
///   github_id        bigint primary key,
///   github_username  text,
///   avatar_url       text,
///   created_at       timestamptz default now().
/// Plus an RLS policy allowing anon INSERT/UPDATE — see README.
pub fn upsert_user(supabase_url: &str, anon_key: &str, user: &GitHubUser) -> Result<()> {
    let trimmed = supabase_url.trim_end_matches('/');
    let url = format!("{trimmed}/rest/v1/users");
    let body = serde_json::json!({
        "github_id": user.id,
        "github_username": user.login,
        "avatar_url": user.avatar_url,
    });
    crate::auth::github::http_agent()
        .post(&url)
        .set("apikey", anon_key)
        .set("Authorization", &format!("Bearer {anon_key}"))
        .set("Content-Type", "application/json")
        .set("Prefer", "resolution=merge-duplicates")
        .send_string(&body.to_string())
        .map_err(|e| anyhow!("supabase upsert failed: {e}"))?;
    Ok(())
}
