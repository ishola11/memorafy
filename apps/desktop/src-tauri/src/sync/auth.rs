use chrono::Utc;
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

pub fn save_session(db: &Database, session: &AuthSession, email: &str) -> Result<(), String> {
    let json = serde_json::to_string(session)
        .map_err(|e| format!("could not serialize session: {e}"))?;
    db.set_setting("auth_session", &json)
        .map_err(|e| e.to_string())?;
    db.set_setting("user_email", email).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_session(db: &Database) -> Result<Option<AuthSession>, rusqlite::Error> {
    let Some(raw) = db.get_setting("auth_session")? else {
        return Ok(None);
    };
    match serde_json::from_str(&raw) {
        Ok(session) => Ok(Some(session)),
        Err(e) => {
            // A corrupt session row means the user is effectively signed out;
            // log it so "why did I get signed out?" reports are debuggable.
            tracing::warn!("stored session is corrupt, treating as signed out: {e}");
            Ok(None)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_expiring_within_margin_counts_as_expired() {
        let session = AuthSession {
            access_token: "t".into(),
            refresh_token: "r".into(),
            user_id: "u".into(),
            expires_at: Utc::now().timestamp() + 30,
        };
        assert!(session_expired(&session));
    }

    #[test]
    fn fresh_session_is_not_expired() {
        let session = AuthSession {
            access_token: "t".into(),
            refresh_token: "r".into(),
            user_id: "u".into(),
            expires_at: Utc::now().timestamp() + 3600,
        };
        assert!(!session_expired(&session));
    }
}
