use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};

use super::auth::{session_from_auth_response, AuthSession, RefreshError};
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

    pub async fn upsert_item(&self, session: &AuthSession, item: &ItemRecord) -> Result<(), String> {
        let cloud = item_to_cloud(session, item);
        let url = format!("{}/items", self.config.rest_url());
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
            return Err(format!("Push failed: {text}"));
        }
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
                "Device registration is not configured. Run services/migrations/006_devices_rpc_only.sql in Supabase SQL Editor, then update to Memora v0.1.6 or later.".into(),
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

/// Builds authenticated request headers. Fails (instead of panicking) if the
/// configured anon key or token contains characters invalid in a header —
/// e.g. a newline from a badly pasted secret.
fn auth_headers(config: &SyncConfig, session: &AuthSession) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "apikey",
        HeaderValue::from_str(&config.anon_key)
            .map_err(|_| "Sync is misconfigured: the Supabase anon key is invalid".to_string())?,
    );
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", session.access_token))
            .map_err(|_| "Session token is invalid — please sign in again".to_string())?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    Ok(headers)
}

fn item_to_cloud(session: &AuthSession, item: &ItemRecord) -> CloudItem {
    CloudItem {
        id: item.id.clone(),
        user_id: session.user_id.clone(),
        kind: item.kind.clone(),
        content_type: item.content_type.clone(),
        display_title: item.display_title.clone(),
        preview_text: item.preview_text.clone(),
        char_count: item.char_count,
        url: item.url.clone(),
        url_title: item.url_title.clone(),
        url_domain: item.url_domain.clone(),
        code_language: item.code_language.clone(),
        line_count: item.line_count,
        // Never push the local absolute filesystem path (leaks the
        // username/directory layout and is meaningless on other devices —
        // there's no Supabase Storage upload yet, so image bytes stay
        // local-only until that lands).
        blob_path: None,
        blob_size: item.blob_size,
        content_hash: item.content_hash.clone(),
        plain_text: item.plain_text.clone(),
        trigger: item.trigger.clone(),
        source_device_id: item.source_device_id.clone(),
        is_pinned: item.is_pinned,
        is_favorited: item.is_favorited,
        created_at: item.created_at.clone(),
        updated_at: item.updated_at.clone(),
        deleted_at: item.deleted_at.clone(),
    }
}
