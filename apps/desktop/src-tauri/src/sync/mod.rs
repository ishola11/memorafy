mod auth;
pub mod client;
mod config;
mod realtime;

pub use auth::AuthSession;
pub use client::{CloudDevice, CloudItem};
pub use config::SyncConfig;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::clipboard::write_clipboard;
use crate::db::{
    Database, SETTING_LAST_AUTH_USER_ID, SETTING_LOCAL_DEVICE_ID, SETTING_LOCAL_DEVICE_NAME,
};
use crate::AppState;

/// Fast cadence while there is pending work; slow fallback when idle.
/// `request_sync()` wakes the loop immediately, so the idle interval is only
/// a safety net — it is not the latency users see.
const SYNC_ACTIVE_INTERVAL: Duration = Duration::from_secs(2);
const SYNC_IDLE_INTERVAL: Duration = Duration::from_secs(30);
/// Incremental pull cadence — recovers changes the realtime socket missed.
const INCREMENTAL_PULL_INTERVAL: Duration = Duration::from_secs(60);
/// Exponential push backoff: 5s, 10s, 20s … capped at 10 minutes.
const BACKOFF_BASE: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_secs(600);
/// Cursor (server `updated_at`) of the newest change we've pulled.
const SETTING_LAST_PULL_CURSOR: &str = "last_pull_cursor";
/// Set once existing plaintext cloud items have been re-pushed encrypted.
const SETTING_E2E_BACKFILL_DONE: &str = "e2e_backfill_done";

#[derive(Clone, Copy)]
struct BackoffEntry {
    failures: u32,
    next_attempt: Instant,
}

pub struct SyncEngine {
    db: Arc<Database>,
    app: AppHandle,
    config: Option<SyncConfig>,
    client: Option<client::SupabaseClient>,
    last_presence: Mutex<Option<Instant>>,
    last_retention_purge: Mutex<Option<Instant>>,
    last_pull: Mutex<Option<Instant>>,
    work_notify: Notify,
    /// Per-entity push backoff (in-memory; resets on restart, which is fine —
    /// a restart is a reasonable moment to retry everything once).
    backoff: Mutex<HashMap<String, BackoffEntry>>,
    /// End-to-end encryption data key, unwrapped at sign-in and cached in
    /// the OS keychain. `None` while signed out or when the key is locked
    /// (e.g. the password was reset elsewhere and no device holds the key).
    dek: Mutex<Option<crate::crypto::Key>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStateDto {
    pub configured: bool,
    pub logged_in: bool,
    pub user_email: Option<String>,
    pub pending_count: i64,
    pub last_sync_at: Option<String>,
    pub cloud_device_count: i64,
    /// "off" (signed out / not configured), "ready", or "locked" (the
    /// encryption key is unavailable — e.g. after a password reset).
    pub e2e_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTransferDto {
    pub item_id: String,
    pub title: String,
    pub source_device: String,
    pub online_devices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignUpResultDto {
    #[serde(flatten)]
    pub state: SyncStateDto,
    /// True when the account was created but the user must click the
    /// confirmation link emailed to them before they can sign in.
    pub needs_email_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncActionResultDto {
    #[serde(flatten)]
    pub state: SyncStateDto,
    pub message: String,
    pub pending_before: i64,
    pub pending_after: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRepairResultDto {
    #[serde(flatten)]
    pub state: SyncStateDto,
    pub message: String,
    pub pending_before: i64,
    pub pending_after: i64,
    pub queue_cleared: i64,
    pub device_rotated: bool,
}

struct PushReport {
    failures: u32,
}

impl SyncEngine {
    pub fn new(db: Arc<Database>, app: AppHandle) -> Self {
        dotenvy::dotenv().ok();
        dotenvy::from_filename(".env").ok();
        dotenvy::from_filename("../.env").ok();
        dotenvy::from_filename("apps/desktop/.env").ok();

        let config = SyncConfig::from_env();
        let client = config.as_ref().map(|c| client::SupabaseClient::new(c.clone()));

        Self {
            db,
            app,
            config,
            client,
            last_presence: Mutex::new(None),
            last_retention_purge: Mutex::new(None),
            last_pull: Mutex::new(None),
            work_notify: Notify::new(),
            backoff: Mutex::new(HashMap::new()),
            dek: Mutex::new(None),
        }
    }

    // ── End-to-end encryption key lifecycle ─────────────────────────────

    pub fn dek(&self) -> Option<crate::crypto::Key> {
        *self.dek.lock()
    }

    fn set_dek(&self, user_id: &str, dek: crate::crypto::Key) {
        *self.dek.lock() = Some(dek);
        if let Err(e) = crate::keychain::store_dek(&crate::crypto::encode_cached_dek(user_id, &dek)) {
            // Non-fatal: the key still works this session; the next sign-in
            // re-unwraps it from the server copy.
            tracing::warn!("could not cache encryption key in keychain: {e}");
        }
        self.maybe_start_encryption_backfill();
    }

    fn clear_dek(&self) {
        *self.dek.lock() = None;
        if let Err(e) = crate::keychain::clear_dek() {
            tracing::debug!("keychain DEK clear: {e}");
        }
    }

    /// Restore the cached key at startup (no password available then).
    fn load_dek_from_keychain(&self) {
        let user_id = match self.db.get_setting(SETTING_LAST_AUTH_USER_ID) {
            Ok(Some(id)) => id,
            _ => return,
        };
        match crate::keychain::load_dek() {
            Ok(Some(cached)) => {
                if let Some(dek) = crate::crypto::decode_cached_dek(&user_id, &cached) {
                    *self.dek.lock() = Some(dek);
                    self.maybe_start_encryption_backfill();
                } else {
                    tracing::warn!("cached encryption key belongs to a different account — ignoring");
                }
            }
            Ok(None) => tracing::info!(
                "no cached encryption key — sync decryption locked until next sign-in"
            ),
            Err(e) => tracing::warn!("could not read cached encryption key: {e}"),
        }
    }

    /// Establish the DEK during sign-in/sign-up, when the password is
    /// available. Handles: first device (generate), returning device
    /// (unwrap), password changed elsewhere with a locally cached key
    /// (auto-heal by re-wrapping), and the truly locked case.
    async fn ensure_dek_with_password(
        &self,
        session: &AuthSession,
        password: &str,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let kek = crate::crypto::derive_kek(password, &session.user_id)?;

        match client.fetch_user_key(session).await? {
            Some(wrapped) => match crate::crypto::unwrap_dek(&kek, &wrapped) {
                Ok(dek) => {
                    self.set_dek(&session.user_id, dek);
                }
                Err(_) => {
                    // The server copy was wrapped under a different password
                    // (reset elsewhere). A device that still holds the key
                    // can heal the account by re-wrapping under the new one.
                    let cached = crate::keychain::load_dek()
                        .ok()
                        .flatten()
                        .and_then(|c| crate::crypto::decode_cached_dek(&session.user_id, &c));
                    match cached {
                        Some(dek) => {
                            let rewrapped = crate::crypto::wrap_dek(&kek, &dek);
                            client.upsert_user_key(session, &rewrapped).await?;
                            self.set_dek(&session.user_id, dek);
                            tracing::info!(
                                "encryption key re-wrapped under the new password (auto-heal)"
                            );
                        }
                        None => {
                            *self.dek.lock() = None;
                            tracing::warn!(
                                "encryption key locked: server key was wrapped under a \
                                 different password and no local copy exists"
                            );
                        }
                    }
                }
            },
            None => {
                let dek = crate::crypto::generate_key();
                let wrapped = crate::crypto::wrap_dek(&kek, &dek);
                client.upsert_user_key(session, &wrapped).await?;
                self.set_dek(&session.user_id, dek);
                tracing::info!("generated a new end-to-end encryption key for this account");
            }
        }
        Ok(())
    }

    /// Settings → "Unlock": the startup path has no password, so a user
    /// whose keychain lost the cached key re-enters it here.
    pub async fn unlock_encryption(&self, password: &str) -> Result<SyncStateDto, String> {
        let session = ensure_session(self).await.map_err(|e| e.to_string())?;
        self.ensure_dek_with_password(&session, password).await?;
        if self.dek().is_none() {
            return Err(
                "That password doesn't match the encryption key. If you reset your password \
                 recently, sign in on a device that still has Memora unlocked — or use \
                 \"Reset sync encryption\" to start a new key."
                    .into(),
            );
        }
        // Items skipped while locked advanced the pull cursor past them —
        // a full bootstrap recovers everything now that we can decrypt.
        self.bootstrap_after_auth(&session).await?;
        self.get_state()
    }

    /// Settings → "Reset sync encryption" (destructive): generates a new
    /// key. Previously synced clips encrypted under the lost key become
    /// permanently unreadable and are purged from this device's view.
    pub async fn reset_encryption(&self, password: &str) -> Result<SyncStateDto, String> {
        let session = ensure_session(self).await.map_err(|e| e.to_string())?;
        let client = self.client.as_ref().ok_or("Supabase not configured")?;

        let kek = crate::crypto::derive_kek(password, &session.user_id)?;
        let dek = crate::crypto::generate_key();
        let wrapped = crate::crypto::wrap_dek(&kek, &dek);
        client.upsert_user_key(&session, &wrapped).await?;
        self.set_dek(&session.user_id, dek);
        // Force a full re-encrypt push of everything this device still has.
        let _ = self.db.delete_setting(SETTING_E2E_BACKFILL_DONE);
        self.maybe_start_encryption_backfill();
        tracing::warn!("sync encryption key was reset — clips from the old key are unrecoverable");
        self.get_state()
    }

    /// Re-wrap the existing DEK when the user changes their password while
    /// signed in (no data loss, unlike an email reset).
    fn rewrap_key_for_new_password(
        &self,
        session: &AuthSession,
        new_password: &str,
    ) -> Result<Option<String>, String> {
        let Some(dek) = self.dek() else {
            return Ok(None);
        };
        let kek = crate::crypto::derive_kek(new_password, &session.user_id)?;
        Ok(Some(crate::crypto::wrap_dek(&kek, &dek)))
    }

    /// Decrypts an incoming cloud item. Returns `None` when the item is
    /// encrypted but can't be decrypted (no key, or key mismatch) — the
    /// caller must skip it rather than store ciphertext as content.
    fn decrypt_incoming(&self, mut item: client::CloudItem) -> Option<client::CloudItem> {
        if !item.encrypted {
            return Some(item);
        }
        let Some(dek) = self.dek() else {
            tracing::debug!("skipping encrypted item {} — encryption key locked", item.id);
            return None;
        };

        let mut failed = false;
        let mut dec = |value: &mut Option<String>| {
            if let Some(v) = value.as_deref() {
                if crate::crypto::is_encrypted_value(v) {
                    match crate::crypto::decrypt_str(&dek, v) {
                        Ok(plain) => *value = Some(plain),
                        Err(_) => failed = true,
                    }
                }
            }
        };
        dec(&mut item.display_title);
        dec(&mut item.preview_text);
        dec(&mut item.url);
        dec(&mut item.url_title);
        dec(&mut item.url_domain);
        dec(&mut item.plain_text);
        dec(&mut item.trigger);

        if failed {
            tracing::warn!("could not decrypt item {} — wrong key generation? skipping", item.id);
            return None;
        }

        // The cloud hash is an HMAC; local dedupe compares plain SHA-256,
        // so recompute the local-format hash from the decrypted content.
        if let Some(text) = item.plain_text.as_deref() {
            item.content_hash =
                crate::clipboard::hash_content(&item.content_type, Some(text), None);
        }
        Some(item)
    }

    /// One-time migration: re-push every local item so plaintext rows
    /// uploaded before E2E encryption get overwritten with ciphertext.
    fn maybe_start_encryption_backfill(&self) {
        let done = self
            .db
            .get_setting(SETTING_E2E_BACKFILL_DONE)
            .ok()
            .flatten()
            .is_some();
        if done {
            return;
        }
        match self.db.mark_all_items_pending() {
            Ok(n) => {
                if let Err(e) = self.db.set_setting(SETTING_E2E_BACKFILL_DONE, "1") {
                    tracing::warn!("could not record encryption backfill: {e}");
                }
                if n > 0 {
                    tracing::info!("re-encrypting {n} previously synced item(s) in the cloud");
                    self.request_sync();
                }
            }
            Err(e) => tracing::warn!("encryption backfill: {e}"),
        }
    }

    /// Wake the sync loop now (called after local mutations). Safe from any
    /// thread; a no-op when nothing is pending.
    pub fn request_sync(&self) {
        self.work_notify.notify_one();
    }

    fn backoff_due(&self, key: &str) -> bool {
        self.backoff
            .lock()
            .get(key)
            .map(|e| Instant::now() >= e.next_attempt)
            .unwrap_or(true)
    }

    fn backoff_record_failure(&self, key: &str) {
        let mut map = self.backoff.lock();
        let failures = map.get(key).map(|e| e.failures + 1).unwrap_or(1);
        let delay = BACKOFF_BASE
            .saturating_mul(2u32.saturating_pow(failures.saturating_sub(1)))
            .min(BACKOFF_MAX);
        map.insert(
            key.to_string(),
            BackoffEntry {
                failures,
                next_attempt: Instant::now() + delay,
            },
        );
        if failures == 1 || failures % 5 == 0 {
            tracing::warn!("push backoff for {key}: attempt {failures}, next retry in {delay:?}");
        }
    }

    fn backoff_clear(&self, key: &str) {
        self.backoff.lock().remove(key);
    }

    fn backoff_reset_all(&self) {
        self.backoff.lock().clear();
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn get_state(&self) -> Result<SyncStateDto, String> {
        let session = auth::load_session(&self.db).map_err(|e| e.to_string())?;
        let pending = self.db.pending_sync_count().map_err(|e| e.to_string())?;
        let last_sync = self
            .db
            .get_setting("last_sync_at")
            .map_err(|e| e.to_string())?;
        let email = self
            .db
            .get_setting("user_email")
            .map_err(|e| e.to_string())?;

        let logged_in = session.is_some();
        let e2e_status = if !self.is_configured() || !logged_in {
            "off"
        } else if self.dek().is_some() {
            "ready"
        } else {
            "locked"
        };

        Ok(SyncStateDto {
            configured: self.is_configured(),
            logged_in,
            user_email: email,
            pending_count: pending,
            last_sync_at: last_sync,
            cloud_device_count: self.db.get_devices().map(|d| d.len() as i64).unwrap_or(0),
            e2e_status: e2e_status.to_string(),
        })
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<SyncStateDto, String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let session = client.login(email, password).await?;
        auth::save_session(&self.db, &session, email)?;
        // Key first, then bootstrap — pulled items need the DEK to decrypt.
        if let Err(e) = self.ensure_dek_with_password(&session, password).await {
            tracing::warn!("encryption key setup at login: {e}");
        }
        self.bootstrap_after_auth(&session).await?;
        let _ = self.sync_tick().await;
        self.get_state()
    }

    pub fn logout(&self) -> Result<SyncStateDto, String> {
        auth::clear_session(&self.db).map_err(|e| e.to_string())?;
        self.clear_dek();
        self.get_state()
    }

    pub async fn sign_up(&self, email: &str, password: &str) -> Result<SignUpResultDto, String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        match client.sign_up(email, password).await? {
            client::SignUpOutcome::SignedIn(session) => {
                auth::save_session(&self.db, &session, email)?;
                if let Err(e) = self.ensure_dek_with_password(&session, password).await {
                    tracing::warn!("encryption key setup at sign-up: {e}");
                }
                self.bootstrap_after_auth(&session).await?;
                let _ = self.sync_tick().await;
                Ok(SignUpResultDto {
                    state: self.get_state()?,
                    needs_email_confirmation: false,
                })
            }
            client::SignUpOutcome::ConfirmationEmailSent => Ok(SignUpResultDto {
                state: self.get_state()?,
                needs_email_confirmation: true,
            }),
        }
    }

    pub async fn resend_confirmation(&self, email: &str) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        client.resend_confirmation(email).await
    }

    pub async fn request_password_reset(&self, email: &str) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        client.request_password_reset(email).await
    }

    pub async fn change_password(&self, new_password: &str) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let session = ensure_session(self).await.map_err(|e| e.to_string())?;

        // Prepare the re-wrapped key BEFORE changing the password so a
        // failure can't leave the account with a key wrapped under a
        // password that no longer exists.
        let rewrapped = self.rewrap_key_for_new_password(&session, new_password)?;
        if rewrapped.is_none() && self.dek().is_none() {
            return Err(
                "Sync encryption is locked on this device — unlock it in Account settings \
                 before changing your password, or old synced clips would become unreadable."
                    .into(),
            );
        }

        client.update_password(&session, new_password).await?;

        if let Some(wrapped) = rewrapped {
            if let Err(e) = client.upsert_user_key(&session, &wrapped).await {
                // Password already changed; retry-able mismatch. The next
                // sign-in auto-heals from this device's cached key.
                tracing::warn!("re-wrap upload after password change failed (auto-heal will fix on next sign-in): {e}");
            }
        }
        Ok(())
    }

    /// Pull from cloud, push pending changes, and refresh local state.
    pub async fn force_sync_now(&self) -> Result<SyncActionResultDto, String> {
        let pending_before = self.db.pending_sync_count().map_err(|e| e.to_string())?;
        let session = auth::load_session(&self.db).map_err(|e| e.to_string())?.ok_or("Sign in to sync")?;
        let session = try_refresh_session(self, session)
            .await
            .map_err(ensure_session_error_message)?;

        // Manual sync expresses user intent: retry everything immediately.
        self.backoff_reset_all();
        self.bootstrap_after_auth(&session).await?;
        let report = self
            .sync_tick()
            .await
            .map_err(|e| format!("Sync failed: {e}"))?;
        let state = self.get_state()?;
        let pending_after = state.pending_count;
        Ok(SyncActionResultDto {
            message: sync_summary_message(pending_before, pending_after, report.failures, false),
            pending_before,
            pending_after,
            state,
        })
    }

    /// Reset device registration, clear stuck queue rows, re-pull from cloud, and push locals.
    pub async fn repair_sync(&self) -> Result<SyncRepairResultDto, String> {
        let pending_before = self.db.pending_sync_count().map_err(|e| e.to_string())?;
        let session = auth::load_session(&self.db).map_err(|e| e.to_string())?.ok_or("Sign in to repair sync")?;
        let session = try_refresh_session(self, session)
            .await
            .map_err(ensure_session_error_message)?;

        self.backoff_reset_all();
        let queue_cleared = self
            .db
            .clear_pending_sync_queue()
            .map_err(|e| e.to_string())?;
        // Deliberately NOT rotating the device id here: unconditional
        // rotation on every repair created a new cloud device row each time
        // (users saw the same machine listed five times). Bootstrap still
        // rotates in the one case that requires it — the id being claimed
        // by another account (Postgres 23505).
        self.bootstrap_after_auth(&session).await?;
        let report = self
            .sync_tick()
            .await
            .map_err(|e| format!("Sync failed after repair: {e}"))?;

        let state = self.get_state()?;
        let pending_after = state.pending_count;
        Ok(SyncRepairResultDto {
            message: sync_summary_message(pending_before, pending_after, report.failures, true),
            pending_before,
            pending_after,
            queue_cleared,
            device_rotated: false,
            state,
        })
    }

    pub fn start(self: Arc<Self>) {
        let engine = self.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    // Sync is unavailable but the local app must keep working.
                    tracing::error!("sync disabled — could not start async runtime: {e}");
                    return;
                }
            };
            rt.block_on(async {
                if engine.is_configured() {
                    engine.load_dek_from_keychain();
                    // Refresh the stored session before bootstrapping — after
                    // the app has been closed for over an hour the saved JWT
                    // is expired and device registration would fail.
                    match ensure_session(&engine).await {
                        Ok(session) => {
                            if let Err(e) = engine.bootstrap_after_auth(&session).await {
                                tracing::warn!("sync bootstrap: {e}");
                            }
                        }
                        Err(EnsureSessionError::NotLoggedIn | EnsureSessionError::NotConfigured) => {}
                        Err(e) => tracing::warn!("sync bootstrap skipped: {e}"),
                    }
                    let rt_engine = engine.clone();
                    tokio::spawn(async move {
                        realtime::run_realtime_loop(rt_engine).await;
                    });
                }

                loop {
                    engine.run_retention_if_due();

                    if let Err(e) = engine.sync_tick().await {
                        tracing::debug!("sync tick: {e}");
                    }
                    if let Err(e) = engine.pull_incremental_if_due().await {
                        tracing::debug!("incremental pull: {e}");
                    }

                    // Fast cadence only while work is pending; otherwise sleep
                    // until woken by request_sync() or the idle fallback.
                    let pending = engine.db.pending_sync_count().unwrap_or(0);
                    let interval = if pending > 0 {
                        SYNC_ACTIVE_INTERVAL
                    } else {
                        SYNC_IDLE_INTERVAL
                    };
                    tokio::select! {
                        _ = engine.work_notify.notified() => {}
                        _ = tokio::time::sleep(interval) => {}
                    }
                }
            });
        });
    }

    /// Pull changes newer than our cursor. Cheap no-op when nothing changed;
    /// the safety net for anything the realtime socket missed (sleep/wake,
    /// dropped connections, expired socket auth).
    async fn pull_incremental_if_due(&self) -> Result<(), String> {
        let due = self
            .last_pull
            .lock()
            .map(|t: Instant| t.elapsed() >= INCREMENTAL_PULL_INTERVAL)
            .unwrap_or(true);
        if !due {
            return Ok(());
        }

        let Some(session) = (match ensure_session(self).await {
            Ok(s) => Some(s),
            Err(EnsureSessionError::NotConfigured | EnsureSessionError::NotLoggedIn) => None,
            Err(EnsureSessionError::AuthExpired) => None,
            Err(EnsureSessionError::Transient(e)) => return Err(e),
        }) else {
            return Ok(());
        };

        *self.last_pull.lock() = Some(Instant::now());
        self.pull_incremental(&session).await
    }

    pub async fn pull_incremental(&self, session: &AuthSession) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let Some(cursor) = self
            .db
            .get_setting(SETTING_LAST_PULL_CURSOR)
            .map_err(|e| e.to_string())?
        else {
            // No cursor until the first successful bootstrap.
            return Ok(());
        };

        let items = client
            .fetch_items_updated_since(session, &cursor, 500)
            .await?;
        if items.is_empty() {
            return Ok(());
        }

        let count = items.len();
        let mut newest_cursor = cursor;
        let mut changed = false;
        for item in items {
            if item.updated_at > newest_cursor {
                newest_cursor = item.updated_at.clone();
            }
            let result = if item.deleted_at.is_some() {
                // Deletions need no decryption — only the id matters.
                self.db
                    .apply_remote_deletion(&item.id, item.deleted_at.as_deref().unwrap_or(""))
            } else {
                let Some(item) = self.decrypt_incoming(item) else {
                    continue;
                };
                self.db.upsert_remote_item(&item)
            };
            match result {
                Ok(()) => changed = true,
                Err(e) => tracing::warn!("incremental apply: {e}"),
            }
        }

        self.db
            .set_setting(SETTING_LAST_PULL_CURSOR, &newest_cursor)
            .map_err(|e| e.to_string())?;

        if changed {
            tracing::info!("incremental pull applied {count} remote change(s)");
            self.db
                .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())
                .map_err(|e| e.to_string())?;
            let _ = self.app.emit("items-updated", ());
        }
        Ok(())
    }

    async fn bootstrap_after_auth(&self, session: &AuthSession) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let (device_id, device_name) = self.prepare_local_device(session)?;
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else {
            "windows"
        };

        match client
            .register_device(session, &device_id, &device_name, platform)
            .await
        {
            Ok(()) => {}
            Err(err) if err.contains("23505") => {
                tracing::info!("device id linked to another account — rotating local device id");
                let (device_id, device_name) = self.rotate_local_device()?;
                client
                    .register_device(session, &device_id, &device_name, platform)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Err(err) => return Err(err),
        }

        self.db
            .set_setting(SETTING_LAST_AUTH_USER_ID, &session.user_id)
            .map_err(|e| e.to_string())?;

        // Devices must exist locally before items (FK: source_device_id)
        self.pull_devices(session, &device_id).await?;

        // Collections before items/item_collections (FK: collection_id, item_id)
        let remote_collections = client.fetch_collections(session).await.map_err(|e| e.to_string())?;
        for collection in remote_collections {
            if let Err(e) = self.db.upsert_remote_collection(&collection) {
                tracing::warn!("pull collection {}: {e}", collection.id);
            }
        }

        let remote_items = client.fetch_recent_items(session, 100).await.map_err(|e| e.to_string())?;
        for item in remote_items {
            let Some(item) = self.decrypt_incoming(item) else {
                continue;
            };
            if let Err(e) = self.db.upsert_remote_item(&item) {
                tracing::warn!("pull item {}: {e}", item.id);
            }
        }

        let remote_links = client.fetch_item_collections(session).await.map_err(|e| e.to_string())?;
        for link in remote_links {
            if let Err(e) = self.db.upsert_remote_item_collection(&link) {
                tracing::warn!("pull item_collection {}:{}: {e}", link.item_id, link.collection_id);
            }
        }

        // Start the incremental-pull cursor slightly in the past so the next
        // pull overlaps the bootstrap window (upserts are idempotent; a small
        // overlap beats a gap and avoids depending on the local clock).
        let cursor = (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        self.db
            .set_setting(SETTING_LAST_PULL_CURSOR, &cursor)
            .map_err(|e| e.to_string())?;

        self.db
            .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())
            .map_err(|e| e.to_string())?;
        let _ = self.app.emit("items-updated", ());
        let _ = self.app.emit("collections-updated", ());
        Ok(())
    }

    /// Pull the cloud device list, upsert it locally, and prune local rows
    /// the cloud no longer knows (duplicates merged by `register_device`).
    async fn pull_devices(
        &self,
        session: &AuthSession,
        current_device_id: &str,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let remote_devices = client.fetch_devices(session).await.map_err(|e| e.to_string())?;
        let remote_ids: Vec<String> = remote_devices.iter().map(|d| d.id.clone()).collect();

        for device in remote_devices {
            if let Err(e) = self.db.upsert_remote_device(&device) {
                tracing::warn!("pull device {}: {e}", device.id);
            }
        }

        match self.db.prune_local_devices_not_in(&remote_ids, current_device_id) {
            Ok(0) => {}
            Ok(n) => tracing::info!("pruned {n} stale local device record(s)"),
            Err(e) => tracing::warn!("device prune: {e}"),
        }
        Ok(())
    }

    /// Refresh the device list from the cloud on demand (used when the
    /// Devices settings page loads). Transient failures are logged, not
    /// surfaced — the UI falls back to the local snapshot.
    pub async fn refresh_devices(&self) {
        let session = match ensure_session(self).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let device_id = self
            .db
            .get_setting(SETTING_LOCAL_DEVICE_ID)
            .ok()
            .flatten()
            .unwrap_or_default();
        if device_id.is_empty() {
            return;
        }
        if let Err(e) = self.pull_devices(&session, &device_id).await {
            tracing::debug!("device refresh: {e}");
        }
    }

    fn prepare_local_device(&self, session: &AuthSession) -> Result<(String, String), String> {
        let last_user = self
            .db
            .get_setting(SETTING_LAST_AUTH_USER_ID)
            .map_err(|e| e.to_string())?;
        if let Some(previous) = last_user.as_ref() {
            if previous != &session.user_id {
                tracing::info!("cloud account changed — rotating local device id");
                return self.rotate_local_device();
            }
        }

        let device_id = self
            .db
            .get_setting(SETTING_LOCAL_DEVICE_ID)
            .map_err(|e| e.to_string())?
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| {
                self.db
                    .ensure_device()
                    .unwrap_or_else(|_| String::new())
            });
        let device_name = self
            .db
            .get_setting(SETTING_LOCAL_DEVICE_NAME)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "My Device".to_string());
        Ok((device_id, device_name))
    }

    fn rotate_local_device(&self) -> Result<(String, String), String> {
        let device_id = self
            .db
            .rotate_local_device()
            .map_err(|e| e.to_string())?;
        let device_name = self
            .db
            .get_device_name(&device_id)
            .map_err(|e| e.to_string())?;
        self.sync_active_device_id(&device_id);
        Ok((device_id, device_name))
    }

    fn sync_active_device_id(&self, device_id: &str) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.set_device_id(device_id.to_string());
        }
    }

    async fn sync_tick(&self) -> Result<PushReport, Box<dyn std::error::Error + Send + Sync>> {
        let mut failures = 0u32;
        let Some(client) = self.client.as_ref() else {
            return Ok(PushReport { failures });
        };

        let Some(session) = auth::load_session(&self.db)? else {
            return Ok(PushReport { failures });
        };

        let session = match try_refresh_session(self, session).await {
            Ok(session) => session,
            Err(EnsureSessionError::AuthExpired) => return Ok(PushReport { failures }),
            Err(EnsureSessionError::NotConfigured | EnsureSessionError::NotLoggedIn) => {
                return Ok(PushReport { failures });
            }
            Err(EnsureSessionError::Transient(e)) => return Err(e.into()),
        };

        // Push order: collections → items → item_collections (cloud FK parents must exist)
        let pending_collections = self.db.list_pending_sync_collections()?;
        for collection in pending_collections {
            let backoff_key = format!("collection:{}", collection.id);
            if !self.backoff_due(&backoff_key) {
                continue;
            }
            let is_deletion = collection.deleted_at.is_some();
            let result = if is_deletion {
                client.delete_collection(&session, &collection.id).await
            } else {
                client.upsert_collection(&session, &collection).await
            };
            match result {
                Ok(()) => {
                    self.backoff_clear(&backoff_key);
                    if let Err(e) = self.db.mark_collection_synced(&collection.id) {
                        tracing::warn!("mark collection synced {}: {e}", collection.id);
                    }
                    self.db.set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
                    let _ = self.app.emit("collections-updated", ());
                }
                Err(e) => {
                    failures += 1;
                    self.backoff_record_failure(&backoff_key);
                    tracing::warn!("push collection {}: {e}", collection.id);
                }
            }
        }

        // Never downgrade to plaintext: if the encryption key is locked,
        // item pushes wait (they stay pending) until Unlock or Reset.
        // Deletions still go through — they carry no content.
        let dek = self.dek();
        let pending = self.db.list_pending_sync_items()?;
        if dek.is_none() && pending.iter().any(|i| i.deleted_at.is_none()) {
            tracing::debug!("item push paused — encryption key locked");
        }
        for item in pending {
            let is_deletion = item.deleted_at.is_some();
            if dek.is_none() && !is_deletion {
                continue;
            }
            let backoff_key = format!("item:{}", item.id);
            if !self.backoff_due(&backoff_key) {
                continue;
            }
            match client.upsert_item(&session, &item, dek.as_ref()).await {
                Ok(()) => {
                    self.backoff_clear(&backoff_key);
                    self.db.mark_synced(&item.id)?;
                    self.db.set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;

                    if is_deletion {
                        continue;
                    }

                    let online: Vec<String> = self
                        .db
                        .get_devices()?
                        .into_iter()
                        .filter(|d| d.is_online && !d.is_current)
                        .map(|d| d.name)
                        .collect();

                    let title = item
                        .display_title
                        .clone()
                        .or(item.preview_text.clone())
                        .unwrap_or_else(|| "Clipboard item".to_string());

                    let _ = self.app.emit(
                        "sync-transfer",
                        SyncTransferDto {
                            item_id: item.id.clone(),
                            title,
                            source_device: item
                                .source_device_name
                                .clone()
                                .unwrap_or_else(|| "This device".to_string()),
                            online_devices: online,
                        },
                    );
                }
                Err(e) => {
                    failures += 1;
                    self.backoff_record_failure(&backoff_key);
                    tracing::warn!("push item {}: {e}", item.id);
                }
            }
        }

        let pending_links = self.db.list_pending_sync_item_collections()?;
        for link in pending_links {
            let backoff_key = format!("link:{}:{}", link.item_id, link.collection_id);
            if !self.backoff_due(&backoff_key) {
                continue;
            }
            match client
                .upsert_item_collection(
                    &session,
                    &client::CloudItemCollection {
                        item_id: link.item_id.clone(),
                        collection_id: link.collection_id.clone(),
                    },
                )
                .await
            {
                Ok(()) => {
                    self.backoff_clear(&backoff_key);
                    if let Err(e) = self
                        .db
                        .mark_item_collection_synced(&link.item_id, &link.collection_id)
                    {
                        tracing::warn!(
                            "mark item_collection synced {}:{}: {e}",
                            link.item_id,
                            link.collection_id
                        );
                    }
                    self.db.set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
                    let _ = self.app.emit("collections-updated", ());
                    let _ = self.app.emit("items-updated", ());
                }
                Err(e) => {
                    if client::is_foreign_key_violation(&e) {
                        tracing::debug!(
                            "defer item_collection {}:{} until parents sync: {e}",
                            link.item_id,
                            link.collection_id
                        );
                    } else {
                        failures += 1;
                        self.backoff_record_failure(&backoff_key);
                        tracing::warn!(
                            "push item_collection {}:{}: {e}",
                            link.item_id,
                            link.collection_id
                        );
                    }
                }
            }
        }

        let pending_link_deletes = self.db.list_pending_item_collection_deletes()?;
        for link in pending_link_deletes {
            let backoff_key = format!("unlink:{}:{}", link.item_id, link.collection_id);
            if !self.backoff_due(&backoff_key) {
                continue;
            }
            match client
                .delete_item_collection(&session, &link.item_id, &link.collection_id)
                .await
            {
                Ok(()) => {
                    self.backoff_clear(&backoff_key);
                    if let Err(e) = self
                        .db
                        .mark_item_collection_synced(&link.item_id, &link.collection_id)
                    {
                        tracing::warn!(
                            "mark item_collection delete synced {}:{}: {e}",
                            link.item_id,
                            link.collection_id
                        );
                    }
                    self.db.set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
                    let _ = self.app.emit("collections-updated", ());
                    let _ = self.app.emit("items-updated", ());
                }
                Err(e) => {
                    failures += 1;
                    self.backoff_record_failure(&backoff_key);
                    tracing::warn!(
                        "delete item_collection {}:{}: {e}",
                        link.item_id,
                        link.collection_id
                    );
                }
            }
        }

        let should_ping = {
            let last = self.last_presence.lock();
            last.map(|t| t.elapsed() > Duration::from_secs(30))
                .unwrap_or(true)
        };
        if should_ping {
            if let Ok(device_id) = self.db.get_setting("local_device_id") {
                if let Some(id) = device_id {
                    let _ = client.update_device_presence(&session, &id).await;
                }
            }
            *self.last_presence.lock() = Some(Instant::now());
        }

        Ok(PushReport { failures })
    }

    pub async fn handle_remote_item(
        &self,
        record: client::CloudItem,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if record.deleted_at.is_some() {
            self.db
                .apply_remote_deletion(&record.id, record.deleted_at.as_deref().unwrap_or(""))?;
            self.db
                .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
            let _ = self.app.emit("items-updated", ());
            return Ok(());
        }

        // Everything below (storage, clipboard write, toast) needs plaintext.
        let Some(record) = self.decrypt_incoming(record) else {
            return Ok(());
        };

        let local_device = self.db.get_setting("local_device_id")?;
        if record.source_device_id.as_deref() == local_device.as_deref() {
            if self.db.item_exists(&record.id)? {
                return Ok(());
            }
        }

        let from_remote = record
            .source_device_id
            .as_deref()
            .zip(local_device.as_deref())
            .is_some_and(|(source, local)| source != local);
        let is_new = !self.db.item_exists(&record.id)?;

        self.db.upsert_remote_item(&record)?;
        self.db
            .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
        let _ = self.app.emit("items-updated", ());

        if from_remote && is_new {
            if let Some(text) = record
                .plain_text
                .as_deref()
                .filter(|t| !t.trim().is_empty())
            {
                if let Some(state) = self.app.try_state::<AppState>() {
                    if let Err(e) = write_clipboard(&state, text) {
                        tracing::warn!("remote clipboard write: {e}");
                    }
                }
            }

            let source_device = record
                .source_device_id
                .as_deref()
                .map(|id| self.remote_device_name(id))
                .unwrap_or_else(|| "Another device".to_string());
            let title = record
                .display_title
                .clone()
                .or(record.preview_text.clone())
                .unwrap_or_else(|| "Clipboard item".to_string());

            let _ = self.app.emit(
                "sync-received",
                SyncTransferDto {
                    item_id: record.id,
                    title,
                    source_device,
                    online_devices: vec![],
                },
            );
        }

        Ok(())
    }

    pub async fn handle_remote_collection(
        &self,
        record: client::CloudCollection,
        event_type: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if event_type == "DELETE" {
            self.db.delete_remote_collection(&record.id)?;
        } else {
            self.db.upsert_remote_collection(&record)?;
        }
        self.db
            .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
        let _ = self.app.emit("collections-updated", ());
        Ok(())
    }

    pub async fn handle_remote_item_collection(
        &self,
        record: client::CloudItemCollection,
        event_type: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if event_type == "DELETE" {
            self.db
                .delete_remote_item_collection(&record.item_id, &record.collection_id)?;
        } else {
            self.db.upsert_remote_item_collection(&record)?;
        }
        self.db
            .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
        let _ = self.app.emit("collections-updated", ());
        let _ = self.app.emit("items-updated", ());
        Ok(())
    }

    fn remote_device_name(&self, device_id: &str) -> String {
        self.db
            .get_devices()
            .ok()
            .and_then(|devices| {
                devices
                    .into_iter()
                    .find(|d| d.id == device_id)
                    .map(|d| d.name)
            })
            .unwrap_or_else(|| "Another device".to_string())
    }

    pub fn run_retention_if_due(&self) {
        let mut last = self.last_retention_purge.lock();
        let due = last
            .map(|t| t.elapsed() > Duration::from_secs(3600))
            .unwrap_or(true);
        if !due {
            return;
        }
        *last = Some(Instant::now());
        drop(last);

        match self.db.purge_expired_history() {
            Ok(0) => {}
            Ok(n) => {
                tracing::info!("retention purge: removed {n} expired history items");
                let _ = self.app.emit("items-updated", ());
            }
            Err(e) => tracing::warn!("retention purge: {e}"),
        }

        // Completed queue rows are audit noise once synced — without pruning
        // the table grows forever.
        match self.db.prune_synced_queue_rows() {
            Ok(0) => {}
            Ok(n) => tracing::debug!("pruned {n} completed sync queue rows"),
            Err(e) => tracing::warn!("queue prune: {e}"),
        }
    }

    pub fn run_retention_now(&self) -> Result<u32, String> {
        let purged = self.db.purge_expired_history().map_err(|e| e.to_string())?;
        if purged > 0 {
            let _ = self.app.emit("items-updated", ());
        }
        *self.last_retention_purge.lock() = Some(Instant::now());
        Ok(purged)
    }

    pub fn config(&self) -> Option<&SyncConfig> {
        self.config.as_ref()
    }

    pub fn client(&self) -> Option<&client::SupabaseClient> {
        self.client.as_ref()
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn app(&self) -> &AppHandle {
        &self.app
    }
}

pub enum EnsureSessionError {
    NotConfigured,
    NotLoggedIn,
    AuthExpired,
    Transient(String),
}

impl std::fmt::Display for EnsureSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnsureSessionError::NotConfigured => write!(f, "Supabase not configured"),
            EnsureSessionError::NotLoggedIn => write!(f, "Not logged in"),
            EnsureSessionError::AuthExpired => write!(f, "Session expired — please sign in again"),
            EnsureSessionError::Transient(msg) => write!(f, "{msg}"),
        }
    }
}

fn sync_summary_message(
    pending_before: i64,
    pending_after: i64,
    push_failures: u32,
    repaired: bool,
) -> String {
    let prefix = if repaired { "Repair finished" } else { "Sync finished" };

    if push_failures > 0 {
        return format!(
            "{prefix}: {push_failures} change(s) failed to upload. {pending_after} still pending — try Repair sync or sign in again."
        );
    }

    if pending_after == 0 {
        if pending_before > 0 {
            return format!("{prefix}: all {pending_before} pending change(s) uploaded.");
        }
        return format!("{prefix}: everything is up to date.");
    }

    if pending_after < pending_before {
        return format!(
            "{prefix}: uploaded {} change(s). {pending_after} still waiting (usually waiting on linked items or collections).",
            pending_before - pending_after
        );
    }

    format!("{prefix}: {pending_after} change(s) still pending.")
}

fn ensure_session_error_message(err: EnsureSessionError) -> String {
    err.to_string()
}

async fn try_refresh_session(
    engine: &SyncEngine,
    session: AuthSession,
) -> Result<AuthSession, EnsureSessionError> {
    if !auth::session_expired(&session) {
        return Ok(session);
    }

    let client = engine.client().ok_or(EnsureSessionError::NotConfigured)?;
    match client.refresh(&session).await {
        Ok(new_session) => {
            let email = engine
                .db()
                .get_setting("user_email")
                .map_err(|e| EnsureSessionError::Transient(e.to_string()))?
                .unwrap_or_default();
            auth::save_session(engine.db(), &new_session, &email)
                .map_err(EnsureSessionError::Transient)?;
            Ok(new_session)
        }
        Err(auth::RefreshError::InvalidSession) => {
            let _ = auth::clear_session(engine.db());
            Err(EnsureSessionError::AuthExpired)
        }
        Err(auth::RefreshError::Network(msg)) => Err(EnsureSessionError::Transient(msg)),
    }
}

pub async fn ensure_session(engine: &SyncEngine) -> Result<AuthSession, EnsureSessionError> {
    let session = auth::load_session(engine.db())
        .map_err(|e| EnsureSessionError::Transient(e.to_string()))?
        .ok_or(EnsureSessionError::NotLoggedIn)?;
    try_refresh_session(engine, session).await
}
