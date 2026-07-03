use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};

use super::auth::{session_from_auth_response, AuthSession, RefreshError};
use super::auth_callback::AUTH_REDIRECT_URL;
use super::config::SyncConfig;
use crate::db::ItemRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudItem {
    pub id: String,
    pub user_id: String,
    pub kind: String,
    pub content_type: String,
    pub display_title: Option<String>,
    pub preview_text: Option<String>,
    pub char_count: Option<i64>,
    pub url: Option<String>,
    pub url_title: Option<String>,
    pub url_domain: Option<String>,
    pub code_language: Option<String>,
    pub line_count: Option<i64>,
    pub blob_path: Option<String>,
    pub blob_size: Option<i64>,
    pub content_hash: String,
    pub plain_text: Option<String>,
    pub trigger: Option<String>,
    pub source_device_id: Option<String>,
    pub is_pinned: bool,
    pub is_favorited: bool,
    /// True when the content fields carry client-side ciphertext.
    #[serde(default)]
    pub encrypted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDevice {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub platform: String,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCollection {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudItemCollection {
    pub item_id: String,
    pub collection_id: String,
}

/// True when Supabase/Postgres rejected a row because a parent FK row is missing.
pub fn is_foreign_key_violation(err: &str) -> bool {
    err.contains("23503") || err.contains("foreign key")
}

const NETWORK_ERROR_MESSAGE: &str =
    "Couldn't reach the sync server. Check your internet connection and try again.";

/// Result of a sign-up attempt, shaped by the Supabase project's
/// email-confirmation setting.
pub enum SignUpOutcome {
    SignedIn(AuthSession),
    ConfirmationEmailSent,
}

fn friendly_signup_error(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            ["error_description", "msg", "message", "error"]
                .iter()
                .find_map(|k| v.get(k).and_then(|s| s.as_str()).map(String::from))
        })
        .unwrap_or_default();

    if detail.contains("already registered") || detail.contains("already been registered") {
        return "An account with this email already exists. Try signing in instead.".to_string();
    }
    if detail.contains("at least") || detail.to_lowercase().contains("password") {
        return "That password doesn't meet the requirements. Use at least 8 characters.".to_string();
    }
    if detail.contains("invalid format") || detail.to_lowercase().contains("valid email") {
        return "Please enter a valid email address.".to_string();
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "Too many attempts. Please wait a minute and try again.".to_string();
    }
    if status.is_server_error() {
        return NETWORK_ERROR_MESSAGE.to_string();
    }
    if detail.is_empty() {
        format!("Sign-up failed ({status}). Please try again.")
    } else {
        format!("Sign-up failed: {detail}")
    }
}

/// Map raw Supabase auth responses to messages a user can act on. The raw
/// body is logged by the caller; the UI only ever sees these.
fn friendly_auth_error(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            ["error_description", "msg", "message", "error"]
                .iter()
                .find_map(|k| v.get(k).and_then(|s| s.as_str()).map(String::from))
        })
        .unwrap_or_default();

    if detail.contains("Invalid login credentials") || detail.contains("invalid_grant") {
        return "Incorrect email or password.".to_string();
    }
    if detail.contains("Email not confirmed") {
        return "Please confirm your email address, then sign in again.".to_string();
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "Too many attempts. Please wait a minute and try again.".to_string();
    }
    if status.is_server_error() {
        return NETWORK_ERROR_MESSAGE.to_string();
    }
    if detail.is_empty() {
        format!("Sign-in failed ({status}). Please try again.")
    } else {
        format!("Sign-in failed: {detail}")
    }
}

pub struct SupabaseClient {
    http: reqwest::Client,
    config: SyncConfig,
}

impl SupabaseClient {
    pub fn new(config: SyncConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    pub fn config(&self) -> &SyncConfig {
        &self.config
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthSession, String> {
        let url = format!("{}/token?grant_type=password", self.config.auth_url());
        let body = serde_json::json!({ "email": email, "password": password });
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.config.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| NETWORK_ERROR_MESSAGE.to_string())?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("login failed ({status}): {text}");
            return Err(friendly_auth_error(status, &text));
        }

        let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        session_from_auth_response(&value)
    }

    /// Creates a new account. Depending on the Supabase project's settings
    /// the response either contains a full session (email confirmation
    /// disabled → user is signed in immediately) or just a user record
    /// (confirmation enabled → they must click the emailed link first).
    pub async fn sign_up(&self, email: &str, password: &str) -> Result<SignUpOutcome, String> {
        let redirect = urlencoding::encode(AUTH_REDIRECT_URL);
        let url = format!(
            "{}/signup?redirect_to={redirect}",
            self.config.auth_url()
        );
        let body = serde_json::json!({ "email": email, "password": password });
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.config.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| NETWORK_ERROR_MESSAGE.to_string())?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("sign-up failed ({status}): {text}");
            return Err(friendly_signup_error(status, &text));
        }

        let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        match session_from_auth_response(&value) {
            Ok(session) => Ok(SignUpOutcome::SignedIn(session)),
            // No access_token in the response means the project requires
            // email confirmation before a session is issued.
            Err(_) => Ok(SignUpOutcome::ConfirmationEmailSent),
        }
    }

    /// Re-sends the sign-up confirmation email.
    pub async fn resend_confirmation(&self, email: &str) -> Result<(), String> {
        let url = format!("{}/resend", self.config.auth_url());
        let body = serde_json::json!({
            "type": "signup",
            "email": email,
            "redirect_to": AUTH_REDIRECT_URL,
        });
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.config.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| NETWORK_ERROR_MESSAGE.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("resend confirmation failed: {text}");
            return Err("Couldn't resend the email right now. Please try again in a minute.".into());
        }
        Ok(())
    }

    /// Sends a password-reset email. Only transport-level failures are
    /// errors — a success response is returned whether or not the account
    /// exists, and callers must word the UI accordingly so this endpoint
    /// can't be used to probe which emails have accounts.
    pub async fn request_password_reset(&self, email: &str) -> Result<(), String> {
        let url = format!("{}/recover", self.config.auth_url());
        let body = serde_json::json!({
            "email": email,
            "redirect_to": AUTH_REDIRECT_URL,
        });
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.config.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| NETWORK_ERROR_MESSAGE.to_string())?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err("Too many reset requests. Please wait a minute and try again.".into());
        }
        if status.is_server_error() {
            return Err(NETWORK_ERROR_MESSAGE.to_string());
        }
        if !status.is_success() {
            // 4xx (e.g. invalid email format) — log it, but keep the UI
            // response generic; don't reveal whether the account exists.
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!("password reset request ({status}): {text}");
        }
        Ok(())
    }

    /// Changes the signed-in user's password. Requires a valid session.
    pub async fn update_password(
        &self,
        session: &AuthSession,
        new_password: &str,
    ) -> Result<(), String> {
        let url = format!("{}/user", self.config.auth_url());
        let body = serde_json::json!({ "password": new_password });
        let resp = self
            .http
            .put(url)
            .headers(auth_headers(&self.config, session)?)
            .json(&body)
            .send()
            .await
            .map_err(|_| NETWORK_ERROR_MESSAGE.to_string())?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("password change failed ({status}): {text}");
            if text.contains("should be different") {
                return Err("The new password must be different from your current one.".into());
            }
            if text.contains("at least") || text.contains("Password") {
                return Err("That password doesn't meet the requirements. Use at least 8 characters.".into());
            }
            return Err(friendly_auth_error(status, &text));
        }
        Ok(())
    }

    pub async fn refresh(&self, session: &AuthSession) -> Result<AuthSession, RefreshError> {
        let url = format!("{}/token?grant_type=refresh_token", self.config.auth_url());
        let body = serde_json::json!({ "refresh_token": session.refresh_token });
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.config.anon_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RefreshError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            // Only a definitive rejection invalidates the stored session.
            // Server errors and rate limits are transient — signing the user
            // out on a 503 would silently break sync until manual re-login.
            if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!("session refresh rejected ({status}): {text}");
                return Err(RefreshError::InvalidSession);
            }
            tracing::debug!("session refresh transient failure ({status}): {text}");
            return Err(RefreshError::Network(format!(
                "Sync server unavailable ({status})"
            )));
        }

        let value: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| RefreshError::Network(e.to_string()))?;
        session_from_auth_response(&value).map_err(RefreshError::Network)
    }

    pub async fn upsert_item(
        &self,
        session: &AuthSession,
        item: &ItemRecord,
        dek: Option<&crate::crypto::Key>,
    ) -> Result<(), String> {
        if item.content_type == "image" {
            if let Some(local) = item.blob_path.as_deref() {
                if std::path::Path::new(local).is_file() {
                    self.upload_item_blob(session, &item.id, local, dek).await?;
                }
            }
        }

        let mut cloud = item_to_cloud(session, item, dek);
        cloud.user_id = session.user_id.clone();

        let rpc_url = format!("{}/rpc/upsert_item", self.config.rest_url());
        let rpc_body = serde_json::json!({ "p": cloud });
        let rpc_resp = self
            .http
            .post(&rpc_url)
            .headers(auth_headers(&self.config, session)?)
            .json(&rpc_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if rpc_resp.status().is_success() {
            return Ok(());
        }

        let rpc_err = rpc_resp.text().await.unwrap_or_default();
        if rpc_missing(&rpc_err) {
            tracing::debug!("upsert_item RPC unavailable, falling back to REST: {rpc_err}");
        } else if !rpc_err.is_empty() {
            return Err(format!("Push failed: {rpc_err}"));
        }

        cloud.user_id = session.user_id.clone();
        let url = format!("{}/items?on_conflict=id", self.config.rest_url());
        let resp = self
            .http
            .post(url)
            .headers(auth_headers(&self.config, session)?)
            .header("Prefer", "resolution=merge-duplicates,return=minimal")
            .json(&cloud)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Push failed: {text}"));
        }
        Ok(())
    }

    pub async fn upload_item_blob(
        &self,
        session: &AuthSession,
        item_id: &str,
        local_path: &str,
        dek: Option<&crate::crypto::Key>,
    ) -> Result<(), String> {
        let raw = std::fs::read(local_path).map_err(|e| format!("read image blob: {e}"))?;
        let body = if let Some(key) = dek {
            crate::crypto::encrypt_blob(key, &raw).into_bytes()
        } else {
            raw
        };
        let key = blob_storage_key(&session.user_id, item_id);
        let url = format!(
            "{}/object/clip-blobs/{}",
            self.config.storage_url(),
            key
        );
        let resp = self
            .http
            .post(&url)
            .headers(base_auth_headers(&self.config, session)?)
            .header("x-upsert", "true")
            .header("Content-Type", "image/png")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            return Ok(());
        }
        let text = resp.text().await.unwrap_or_default();
        Err(format!("Image upload failed: {text}"))
    }

    pub async fn download_item_blob(
        &self,
        session: &AuthSession,
        item_id: &str,
        dest_path: &std::path::Path,
        dek: Option<&crate::crypto::Key>,
    ) -> Result<(), String> {
        let key = blob_storage_key(&session.user_id, item_id);
        let url = format!(
            "{}/object/clip-blobs/{}",
            self.config.storage_url(),
            key
        );
        let resp = self
            .http
            .get(&url)
            .headers(base_auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err("Image not uploaded yet from the source device.".into());
        }
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Image download failed: {text}"));
        }
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        let png = if let Some(key) = dek {
            if bytes.starts_with(b"mem1:") {
                let encoded = std::str::from_utf8(&bytes)
                    .map_err(|_| "encrypted blob is not valid UTF-8".to_string())?;
                crate::crypto::decrypt_blob(key, encoded)
                    .map_err(|e| format!("decrypt image blob: {e}"))?
            } else {
                bytes.to_vec()
            }
        } else {
            bytes.to_vec()
        };
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(dest_path, png).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn fetch_recent_items(
        &self,
        session: &AuthSession,
        limit: i64,
    ) -> Result<Vec<CloudItem>, String> {
        let url = format!(
            "{}/items?deleted_at=is.null&order=created_at.desc&limit={limit}",
            self.config.rest_url()
        );
        let resp = self
            .http
            .get(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Fetch items failed: {text}"));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    /// Items changed since `cursor` (RFC 3339), oldest first, **including
    /// soft-deleted rows** so deletions propagate. Backbone of incremental
    /// pull — recovers anything missed while the realtime socket was down.
    pub async fn fetch_items_updated_since(
        &self,
        session: &AuthSession,
        cursor: &str,
        limit: i64,
    ) -> Result<Vec<CloudItem>, String> {
        let url = format!(
            "{}/items?updated_at=gt.{}&order=updated_at.asc&limit={limit}",
            self.config.rest_url(),
            urlencoding::encode(cursor)
        );
        let resp = self
            .http
            .get(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Incremental fetch failed: {text}"));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    /// The user's wrapped (password-encrypted) data key, if one exists.
    pub async fn fetch_user_key(&self, session: &AuthSession) -> Result<Option<String>, String> {
        let url = format!(
            "{}/user_encryption_keys?select=wrapped_dek&limit=1",
            self.config.rest_url()
        );
        let resp = self
            .http
            .get(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            if text.contains("42P01") || text.contains("does not exist") {
                return Err(
                    "Encryption key storage is not configured on the server. Apply the migrations in services/supabase/migrations.".into(),
                );
            }
            return Err(format!("Fetch encryption key failed: {text}"));
        }

        let rows: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
        Ok(rows
            .first()
            .and_then(|r| r.get("wrapped_dek"))
            .and_then(|v| v.as_str())
            .map(String::from))
    }

    pub async fn upsert_user_key(
        &self,
        session: &AuthSession,
        wrapped_dek: &str,
    ) -> Result<(), String> {
        let url = format!("{}/user_encryption_keys", self.config.rest_url());
        let body = serde_json::json!({
            "user_id": session.user_id,
            "wrapped_dek": wrapped_dek,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        let resp = self
            .http
            .post(url)
            .headers(auth_headers(&self.config, session)?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Store encryption key failed: {text}"));
        }
        Ok(())
    }

    pub async fn register_device(
        &self,
        session: &AuthSession,
        device_id: &str,
        name: &str,
        platform: &str,
    ) -> Result<(), String> {
        let url = format!("{}/rpc/register_device", self.config.rest_url());
        let resp = self
            .http
            .post(url)
            .headers(auth_headers(&self.config, session)?)
            .json(&serde_json::json!({
                "device_id": device_id,
                "device_name": name,
                "device_platform": platform,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            return Ok(());
        }

        let text = resp.text().await.unwrap_or_default();
        if text.contains("42883") || text.contains("does not exist") {
            return Err(
                "Device registration is not configured on the server. Apply the schema in services/supabase/migrations (or run the baseline migration in the Supabase SQL Editor).".into(),
            );
        }
        Err(format!("Register device failed: {text}"))
    }

    pub async fn update_device_presence(
        &self,
        session: &AuthSession,
        device_id: &str,
    ) -> Result<(), String> {
        let url = format!("{}/rpc/touch_device", self.config.rest_url());
        let resp = self
            .http
            .post(url)
            .headers(auth_headers(&self.config, session)?)
            .json(&serde_json::json!({ "device_id": device_id }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!("touch_device: {text}");
        }
        Ok(())
    }

    pub async fn fetch_devices(&self, session: &AuthSession) -> Result<Vec<CloudDevice>, String> {
        let url = format!("{}/devices?order=name.asc", self.config.rest_url());
        let resp = self
            .http
            .get(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Fetch devices failed: {text}"));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    pub async fn upsert_collection(
        &self,
        session: &AuthSession,
        collection: &crate::db::CollectionRecord,
    ) -> Result<(), String> {
        let cloud = CloudCollection {
            id: collection.id.clone(),
            user_id: session.user_id.clone(),
            name: collection.name.clone(),
            color: collection.color.clone(),
            icon: collection.icon.clone(),
            sort_order: collection.sort_order,
            created_at: collection.created_at.clone(),
        };
        let url = format!("{}/collections", self.config.rest_url());
        let resp = self
            .http
            .post(url)
            .headers(auth_headers(&self.config, session)?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(&cloud)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Push collection failed: {text}"));
        }
        Ok(())
    }

    pub async fn delete_collection(&self, session: &AuthSession, id: &str) -> Result<(), String> {
        let url = format!("{}/collections?id=eq.{}", self.config.rest_url(), id);
        let resp = self
            .http
            .delete(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Delete collection failed: {text}"));
        }
        Ok(())
    }

    pub async fn fetch_collections(
        &self,
        session: &AuthSession,
    ) -> Result<Vec<CloudCollection>, String> {
        let url = format!("{}/collections?order=sort_order.asc", self.config.rest_url());
        let resp = self
            .http
            .get(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Fetch collections failed: {text}"));
        }

        resp.json().await.map_err(|e| e.to_string())
    }

    pub async fn upsert_item_collection(
        &self,
        session: &AuthSession,
        link: &CloudItemCollection,
    ) -> Result<(), String> {
        let url = format!("{}/item_collections", self.config.rest_url());
        let resp = self
            .http
            .post(url)
            .headers(auth_headers(&self.config, session)?)
            .header("Prefer", "resolution=merge-duplicates")
            .json(link)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Push item_collection failed: {text}"));
        }
        Ok(())
    }

    pub async fn delete_item_collection(
        &self,
        session: &AuthSession,
        item_id: &str,
        collection_id: &str,
    ) -> Result<(), String> {
        let url = format!(
            "{}/item_collections?item_id=eq.{}&collection_id=eq.{}",
            self.config.rest_url(),
            item_id,
            collection_id
        );
        let resp = self
            .http
            .delete(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Delete item_collection failed: {text}"));
        }
        Ok(())
    }

    pub async fn fetch_item_collections(
        &self,
        session: &AuthSession,
    ) -> Result<Vec<CloudItemCollection>, String> {
        let url = format!("{}/item_collections", self.config.rest_url());
        let resp = self
            .http
            .get(url)
            .headers(auth_headers(&self.config, session)?)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Fetch item_collections failed: {text}"));
        }

        resp.json().await.map_err(|e| e.to_string())
    }
}

/// PostgREST signals a missing RPC with 42883 (Postgres) or PGRST202 (schema cache).
fn rpc_missing(err: &str) -> bool {
    err.contains("42883")
        || err.contains("PGRST202")
        || err.contains("Could not find the function")
        || err.contains("does not exist")
}

pub fn blob_storage_key(user_id: &str, item_id: &str) -> String {
    format!("{user_id}/{item_id}.png")
}

/// Builds authenticated request headers. Fails (instead of panicking) if the
/// configured anon key or token contains characters invalid in a header —
/// e.g. a newline from a badly pasted secret.
/// apikey + Bearer token only (for Storage uploads/downloads).
fn base_auth_headers(config: &SyncConfig, session: &AuthSession) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "apikey",
        HeaderValue::from_str(&config.anon_key)
            .map_err(|_| "Sync is misconfigured: the Supabase anon key is invalid".to_string())?,
    );
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", session.access_token))
            .map_err(|_| "Session token is invalid. Please sign in again.".to_string())?,
    );
    Ok(headers)
}

/// REST/RPC requests that send JSON bodies.
fn auth_headers(config: &SyncConfig, session: &AuthSession) -> Result<HeaderMap, String> {
    let mut headers = base_auth_headers(config, session)?;
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    Ok(headers)
}

/// Builds the cloud payload. With a DEK, all content-bearing fields are
/// encrypted client-side and `content_hash` becomes an HMAC — the server
/// stores nothing that reveals what the user copied (or lets it test
/// equality against guesses). Structural metadata (kind, flags, timestamps)
/// stays plaintext; the server needs it for ordering and soft-deletes.
fn item_to_cloud(session: &AuthSession, item: &ItemRecord, dek: Option<&crate::crypto::Key>) -> CloudItem {
    use crate::crypto::{encrypt_str, keyed_content_hash};

    let enc = |value: &Option<String>| -> Option<String> {
        match (dek, value) {
            (Some(key), Some(v)) => Some(encrypt_str(key, v)),
            _ => value.clone(),
        }
    };

    CloudItem {
        id: item.id.clone(),
        user_id: session.user_id.clone(),
        kind: item.kind.clone(),
        content_type: item.content_type.clone(),
        display_title: enc(&item.display_title),
        preview_text: enc(&item.preview_text),
        char_count: item.char_count,
        url: enc(&item.url),
        url_title: enc(&item.url_title),
        url_domain: enc(&item.url_domain),
        code_language: item.code_language.clone(),
        line_count: item.line_count,
        blob_path: if item.content_type == "image"
            && item
                .blob_path
                .as_deref()
                .is_some_and(|p| std::path::Path::new(p).is_file())
        {
            Some(blob_storage_key(&session.user_id, &item.id))
        } else {
            None
        },
        blob_size: item.blob_size,
        content_hash: match dek {
            Some(key) => keyed_content_hash(key, &item.content_hash),
            None => item.content_hash.clone(),
        },
        plain_text: enc(&item.plain_text),
        trigger: enc(&item.trigger),
        source_device_id: item.source_device_id.clone(),
        is_pinned: item.is_pinned,
        is_favorited: item.is_favorited,
        encrypted: dek.is_some(),
        created_at: item.created_at.clone(),
        updated_at: item.updated_at.clone(),
        deleted_at: item.deleted_at.clone(),
    }
}
