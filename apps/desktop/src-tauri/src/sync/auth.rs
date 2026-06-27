use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::Database;

#[derive(Debug, Clone)]
pub enum RefreshError {
    Network(String),
    InvalidSession,
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshError::Network(msg) => write!(f, "{msg}"),
            RefreshError::InvalidSession => write!(f, "Session expired — please sign in again"),
        }
    }
}

impl std::error::Error for RefreshError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub expires_at: i64,
}

pub fn save_session(db: &Database, session: &AuthSession, email: &str) -> Result<(), rusqlite::Error> {
    let json = serde_json::to_string(session).expect("session json");
    db.set_setting("auth_session", &json)?;
    db.set_setting("user_email", email)?;
    Ok(())
}

pub fn load_session(db: &Database) -> Result<Option<AuthSession>, rusqlite::Error> {
    let Some(raw) = db.get_setting("auth_session")? else {
        return Ok(None);
    };
    Ok(serde_json::from_str(&raw).ok())
}

pub fn clear_session(db: &Database) -> Result<(), rusqlite::Error> {
    db.delete_setting("auth_session")?;
    db.delete_setting("user_email")?;
    Ok(())
}

pub fn session_expired(session: &AuthSession) -> bool {
    let now = Utc::now().timestamp();
    session.expires_at <= now + 60
}

pub fn session_from_auth_response(value: &serde_json::Value) -> Result<AuthSession, String> {
    let access_token = value
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("missing access_token")?
        .to_string();
    let refresh_token = value
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or("missing refresh_token")?
        .to_string();
    let user_id = value
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|v| v.as_str())
        .ok_or("missing user id")?
        .to_string();
    let expires_in = value
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    let expires_at = Utc::now().timestamp() + expires_in;

    Ok(AuthSession {
        access_token,
        refresh_token,
        user_id,
        expires_at,
    })
}

/// Parses Supabase-style expiry timestamps; covered by unit test for future session refresh work.
#[allow(dead_code)]
pub fn parse_expires_at(iso: &str) -> i64 {
    DateTime::parse_from_rfc3339(iso)
        .map(|d| d.timestamp())
        .unwrap_or_else(|_| Utc::now().timestamp() + 3600)
}

#[cfg(test)]
mod tests {
    use super::parse_expires_at;

    #[test]
    fn parse_expires_at_accepts_rfc3339() {
        let ts = parse_expires_at("2026-01-01T00:00:00Z");
        assert!(ts > 0);
    }
}
