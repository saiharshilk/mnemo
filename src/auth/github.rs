use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const USER_AGENT: &str = "mnemo";
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const SLOW_DOWN_BUMP: Duration = Duration::from_secs(5);

/// All GitHub (and Supabase) calls go through this agent so a stuck network
/// call can't freeze the UI thread for ureq's 30s default.
pub fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(HTTP_TIMEOUT)
        .build()
}

#[derive(Debug)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds GitHub asks us to wait between polls. May grow on `slow_down`.
    pub interval: u64,
}

#[derive(Deserialize)]
struct RawDeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
    #[allow(dead_code)]
    expires_in: u64,
}

fn parse_json(resp: ureq::Response) -> Result<serde_json::Value> {
    let body = resp
        .into_string()
        .map_err(|e| anyhow!("failed to read response body: {e}"))?;
    serde_json::from_str(&body).map_err(|e| anyhow!("invalid json response: {e} (body: {body})"))
}

fn parse_json_or_null(resp: ureq::Response) -> serde_json::Value {
    parse_json(resp).unwrap_or(serde_json::Value::Null)
}

pub fn request_device_code(client_id: &str) -> Result<DeviceCodeResponse> {
    let resp = http_agent()
        .post("https://github.com/login/device/code")
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .send_form(&[("client_id", client_id), ("scope", "read:user")])
        .map_err(|e| anyhow!("github /login/device/code request failed: {e}"))?;
    let raw: RawDeviceCode = serde_json::from_str(
        &resp
            .into_string()
            .map_err(|e| anyhow!("failed to read device code body: {e}"))?,
    )
    .map_err(|e| anyhow!("invalid /login/device/code response: {e}"))?;
    Ok(DeviceCodeResponse {
        device_code: raw.device_code,
        user_code: raw.user_code,
        verification_uri: raw.verification_uri,
        interval: raw.interval.unwrap_or(5).max(1),
    })
}

#[derive(Debug)]
pub enum PollResult {
    Success(String),
    Error(String),
    Cancelled,
}

/// Polls GitHub for the access token, sleeping `*interval_secs` between attempts.
/// On `slow_down`, the caller's `interval_secs` is bumped by 5 per the GitHub spec
/// so we don't trigger the rate limiter. Returns Success on `access_token`, Error on
/// a non-transient failure (e.g. `expired_token`), or Cancelled if the cancel flag fired.
pub fn poll_for_token(
    client_id: &str,
    device_code: &str,
    interval_secs: &mut u64,
    cancel: &AtomicBool,
) -> PollResult {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return PollResult::Cancelled;
        }
        thread::sleep(Duration::from_secs(*interval_secs));
        if cancel.load(Ordering::Relaxed) {
            return PollResult::Cancelled;
        }

        let post = http_agent()
            .post("https://github.com/login/oauth/access_token")
            .set("Accept", "application/json")
            .set("User-Agent", USER_AGENT)
            .send_form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ]);

        let body: serde_json::Value = match post {
            Ok(r) => parse_json_or_null(r),
            Err(ureq::Error::Status(_, r)) => parse_json_or_null(r),
            Err(_) => continue, // transport error: sleep and retry until expires_in
        };

        if let Some(t) = body.get("access_token").and_then(|v| v.as_str()) {
            return PollResult::Success(t.to_string());
        }
        match body.get("error").and_then(|v| v.as_str()).unwrap_or("") {
            "" | "authorization_pending" => continue,
            "slow_down" => {
                *interval_secs = interval_secs.saturating_add(SLOW_DOWN_BUMP.as_secs());
                continue;
            }
            "expired_token" => {
                return PollResult::Error(
                    "the device code expired — please retry".to_string(),
                );
            }
            "access_denied" => {
                return PollResult::Error(
                    "authorization was denied in the browser".to_string(),
                );
            }
            other => return PollResult::Error(format!("github returned error: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct RawUser {
    id: i64,
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

pub fn fetch_user(token: &str) -> Result<GitHubUser> {
    let resp = http_agent()
        .get("https://api.github.com/user")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| anyhow!("github /user request failed: {e}"))?;
    let raw: RawUser = serde_json::from_str(
        &resp
            .into_string()
            .map_err(|e| anyhow!("failed to read /user body: {e}"))?,
    )
    .map_err(|e| anyhow!("invalid /user response: {e}"))?;
    Ok(GitHubUser {
        id: raw.id,
        login: raw.login,
        avatar_url: raw.avatar_url,
    })
}

#[derive(Debug)]
pub enum TokenStatus {
    Valid,
    Invalid,
    Inconclusive(anyhow::Error),
}

/// Lightweight check used at startup to decide whether a persisted session
/// is still valid. Distinguishes a revoked token (401) from transient
/// network failures so a flaky connection doesn't force re-authentication.
pub fn check_token(token: &str) -> TokenStatus {
    match http_agent()
        .get("https://api.github.com/user")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", USER_AGENT)
        .call()
    {
        Ok(_) => TokenStatus::Valid,
        Err(ureq::Error::Status(401, _)) => TokenStatus::Invalid,
        Err(e) => TokenStatus::Inconclusive(e.into()),
    }
}
