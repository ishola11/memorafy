mod auth;
pub mod client;
mod config;
mod realtime;

pub use auth::AuthSession;
pub use client::{CloudDevice, CloudItem};
pub use config::SyncConfig;

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::write_clipboard;
use crate::db::Database;
use crate::AppState;

pub struct SyncEngine {
    db: Arc<Database>,
    app: AppHandle,
    config: Option<SyncConfig>,
    client: Option<client::SupabaseClient>,
    last_presence: Mutex<Option<Instant>>,
    last_retention_purge: Mutex<Option<Instant>>,
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncTransferDto {
    pub item_id: String,
    pub title: String,
    pub source_device: String,
    pub online_devices: Vec<String>,
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
        }
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

        Ok(SyncStateDto {
            configured: self.is_configured(),
            logged_in: session.is_some(),
            user_email: email,
            pending_count: pending,
            last_sync_at: last_sync,
            cloud_device_count: self.db.get_devices().map(|d| d.len() as i64).unwrap_or(0),
        })
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<SyncStateDto, String> {
        let client = self.client.as_ref().ok_or("Supabase not configured")?;
        let session = client.login(email, password).await.map_err(|e| e.to_string())?;
        auth::save_session(&self.db, &session, email).map_err(|e| e.to_string())?;
        self.bootstrap_after_auth(&session).await?;
        self.get_state()
    }

    pub fn logout(&self) -> Result<SyncStateDto, String> {
        auth::clear_session(&self.db).map_err(|e| e.to_string())?;
        self.get_state()
    }

    /// Pull from cloud, push pending changes, and refresh local state.
    pub async fn force_sync_now(&self) -> Result<SyncStateDto, String> {
        let session = auth::load_session(&self.db).map_err(|e| e.to_string())?.ok_or("Sign in to sync")?;
        let session = try_refresh_session(self, session)
            .await
            .map_err(ensure_session_error_message)?;

        self.bootstrap_after_auth(&session).await?;
        self.sync_tick()
            .await
            .map_err(|e| format!("Sync failed: {e}"))?;
        self.get_state()
    }

    pub fn start(self: Arc<Self>) {
        let engine = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async {
                if engine.is_configured() {
                    if let Ok(Some(session)) = auth::load_session(&engine.db) {
                        if let Err(e) = engine.bootstrap_after_auth(&session).await {
                            tracing::warn!("sync bootstrap: {e}");
                        }
                        let rt_engine = engine.clone();
                        tokio::spawn(async move {
                            realtime::run_realtime_loop(rt_engine).await;
                        });
                    }
                }

                loop {
                    engine.run_retention_if_due();
                    if let Err(e) = engine.sync_tick().await {
                        tracing::debug!("sync tick: {e}");
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
        });
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
            .set_setting(
                crate::db::models::SETTING_LAST_AUTH_USER_ID,
                &session.user_id,
            )
            .map_err(|e| e.to_string())?;

        // Devices must exist locally before items (FK: source_device_id)
        let remote_devices = client.fetch_devices(session).await.map_err(|e| e.to_string())?;
        for device in remote_devices {
            if let Err(e) = self.db.upsert_remote_device(&device) {
                tracing::warn!("pull device {}: {e}", device.id);
            }
        }

        // Collections before items/item_collections (FK: collection_id, item_id)
        let remote_collections = client.fetch_collections(session).await.map_err(|e| e.to_string())?;
        for collection in remote_collections {
            if let Err(e) = self.db.upsert_remote_collection(&collection) {
                tracing::warn!("pull collection {}: {e}", collection.id);
            }
        }

        let remote_items = client.fetch_recent_items(session, 100).await.map_err(|e| e.to_string())?;
        for item in remote_items {
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

        self.db
            .set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())
            .map_err(|e| e.to_string())?;
        let _ = self.app.emit("items-updated", ());
        let _ = self.app.emit("collections-updated", ());
        Ok(())
    }

    fn prepare_local_device(&self, session: &AuthSession) -> Result<(String, String), String> {
        let last_user = self
            .db
            .get_setting(crate::db::models::SETTING_LAST_AUTH_USER_ID)
            .map_err(|e| e.to_string())?;
        if let Some(previous) = last_user.as_ref() {
            if previous != &session.user_id {
                tracing::info!("cloud account changed — rotating local device id");
                return self.rotate_local_device();
            }
        }

        let device_id = self
            .db
            .get_setting(crate::db::models::SETTING_LOCAL_DEVICE_ID)
            .map_err(|e| e.to_string())?
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| {
                self.db
                    .ensure_device()
                    .unwrap_or_else(|_| String::new())
            });
        let device_name = self
            .db
            .get_setting(crate::db::models::SETTING_LOCAL_DEVICE_NAME)
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

    async fn sync_tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(client) = self.client.as_ref() else {
            return Ok(());
        };

        let Some(session) = auth::load_session(&self.db)? else {
            return Ok(());
        };

        let session = match try_refresh_session(self, session).await {
            Ok(session) => session,
            Err(EnsureSessionError::AuthExpired) => return Ok(()),
            Err(EnsureSessionError::NotConfigured | EnsureSessionError::NotLoggedIn) => return Ok(()),
            Err(EnsureSessionError::Transient(e)) => return Err(e.into()),
        };

        // Push order: collections → items → item_collections (cloud FK parents must exist)
        let pending_collections = self.db.list_pending_sync_collections()?;
        for collection in pending_collections {
            let is_deletion = collection.deleted_at.is_some();
            let result = if is_deletion {
                client.delete_collection(&session, &collection.id).await
            } else {
                client.upsert_collection(&session, &collection).await
            };
            match result {
                Ok(()) => {
                    if let Err(e) = self.db.mark_collection_synced(&collection.id) {
                        tracing::warn!("mark collection synced {}: {e}", collection.id);
                    }
                    self.db.set_setting("last_sync_at", &chrono::Utc::now().to_rfc3339())?;
                    let _ = self.app.emit("collections-updated", ());
                }
                Err(e) => tracing::warn!("push collection {}: {e}", collection.id),
            }
        }

        let pending = self.db.list_pending_sync_items()?;
        for item in pending {
            let is_deletion = item.deleted_at.is_some();
            match client.upsert_item(&session, &item).await {
                Ok(()) => {
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
                Err(e) => tracing::warn!("push item {}: {e}", item.id),
            }
        }

        let pending_links = self.db.list_pending_sync_item_collections()?;
        for link in pending_links {
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
            match client
                .delete_item_collection(&session, &link.item_id, &link.collection_id)
                .await
            {
                Ok(()) => {
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
                Err(e) => tracing::warn!(
                    "delete item_collection {}:{}: {e}",
                    link.item_id,
                    link.collection_id
                ),
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

        Ok(())
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
                .map_err(|e| EnsureSessionError::Transient(e.to_string()))?;
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
