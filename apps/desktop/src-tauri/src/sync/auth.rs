use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::keychain;

/// SQLite key used before the OS keychain was adopted, and as a fallback
/// when no OS credential backend is available.
const LEGACY_SETTING_KEY: &str = "auth_session";

#[derive(Debug, Clone)]
pub enum RefreshError {
    Network(String),
    InvalidSession,
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshError::Network(msg) => write!(f, "{msg}"),
            RefreshError::InvalidSession => write!(f, "Session expired. Please sign in again."),
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

/// Access/refresh tokens are long-lived credentials, so they live in the OS
/// keychain rather than plaintext SQLite. If no credential backend is
/// available (e.g. a headless Linux box with no Secret Service), we fall
/// back to the local database rather than breaking sign-in entirely — this
/// keeps the app usable while being the exception, not the rule.
pub fn save_session(db: &Database, session: &AuthSession, email: &str) -> Result<(), String> {
    let json = serde_json::to_string(session)
        .map_err(|e| format!("could not serialize session: {e}"))?;

    if keychain::prefer_local_storage() {
        db.set_setting(LEGACY_SETTING_KEY, &json)
            .map_err(|e| e.to_string())?;
    } else {
        match keychain::store(&json) {
            Ok(()) => {
                let _ = db.delete_setting(LEGACY_SETTING_KEY);
            }
            Err(e) => {
                tracing::warn!("keychain unavailable, falling back to local storage: {e}");
                db.set_setting(LEGACY_SETTING_KEY, &json)
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    db.set_setting("user_email", email).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_session(db: &Database) -> Result<Option<AuthSession>, String> {
    let raw = if keychain::prefer_local_storage() {
        db.get_setting(LEGACY_SETTING_KEY).map_err(|e| e.to_string())?
    } else {
        match keychain::load() {
            Ok(Some(json)) => Some(json),
            Ok(None) => migrate_legacy_session(db)?,
            Err(e) => {
                tracing::warn!("keychain unavailable, reading local fallback: {e}");
                db.get_setting(LEGACY_SETTING_KEY).map_err(|e| e.to_string())?
            }
        }
    };

    let Some(raw) = raw else {
        return Ok(None);
    };

    match serde_json::from_str(&raw) {
        Ok(session) => Ok(Some(session)),
        Err(e) => {
            // A corrupt session means the user is effectively signed out;
            // log it so "why did I get signed out?" reports are debuggable.
            tracing::warn!("stored session is corrupt, treating as signed out: {e}");
            Ok(None)
        }
    }
}

/// One-time upgrade path for installs that saved a session before Memora
/// used the OS keychain (or that fell back to SQLite on a prior run).
/// Leaves the plaintext row in place if the keychain write fails, so a
/// transient backend issue can't lose the user's session.
fn migrate_legacy_session(db: &Database) -> Result<Option<String>, String> {
    if keychain::prefer_local_storage() {
        return db.get_setting(LEGACY_SETTING_KEY).map_err(|e| e.to_string());
    }
    let Some(raw) = db.get_setting(LEGACY_SETTING_KEY).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    match keychain::store(&raw) {
        Ok(()) => {
            let _ = db.delete_setting(LEGACY_SETTING_KEY);
            tracing::info!("migrated auth session from local storage to the system keychain");
        }
        Err(e) => tracing::warn!("could not migrate session to keychain, keeping local copy: {e}"),
    }
    Ok(Some(raw))
}

pub fn clear_session(db: &Database) -> Result<(), String> {
    if !keychain::prefer_local_storage() {
        if let Err(e) = keychain::clear() {
            tracing::debug!("keychain clear: {e}");
        }
    }
    db.delete_setting(LEGACY_SETTING_KEY).map_err(|e| e.to_string())?;
    db.delete_setting("user_email").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn session_expired(session: &AuthSession) -> bool {
    let now = Utc::now().timestamp();
    session.expires_at <= now + 60
}

/// Decode the JWT payload from a Supabase access token (no signature verification —
/// the token was issued by Supabase over HTTPS and we use it immediately).
pub fn claims_from_access_token(token: &str) -> Result<(String, Option<String>), String> {
    let payload_b64 = token
        .split('.')
        .nth(1)
        .ok_or("Invalid access token")?;
    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        payload_b64,
    )
    .map_err(|e| format!("Invalid access token payload: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid access token payload: {e}"))?;
    let user_id = value
        .get("sub")
        .and_then(|v| v.as_str())
        .ok_or("Access token missing user id")?
        .to_string();
    let email = value
        .get("email")
        .and_then(|v| v.as_str())
        .map(String::from);
    Ok((user_id, email))
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
