use std::path::{Path, PathBuf};
use parking_lot::Mutex;

use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use super::models::*;
use crate::timeline;

fn scope_label(scope: ClearHistoryScope) -> &'static str {
    match scope {
        ClearHistoryScope::Local => "local",
        ClearHistoryScope::Everywhere => "everywhere",
    }
}

fn mode_label(mode: ClearHistoryMode) -> &'static str {
    match mode {
        ClearHistoryMode::Expired => "expired",
        ClearHistoryMode::All => "all",
    }
}

pub struct Database {
    conn: Mutex<Connection>,
    blobs_dir: PathBuf,
}

impl Database {
    pub fn open(db_path: PathBuf) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let blobs_dir = db_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("blobs");
        std::fs::create_dir_all(&blobs_dir).ok();

        let db = Self {
            conn: Mutex::new(conn),
            blobs_dir,
        };
        db.migrate()?;
        db.seed_defaults()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        let sql = include_str!("migrations/001_initial.sql");
        self.conn.lock().execute_batch(sql)?;
        self.apply_schema_patches()?;
        Ok(())
    }

    fn apply_schema_patches(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let patches = [
            "ALTER TABLE collections ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'synced'",
            "ALTER TABLE collections ADD COLUMN deleted_at TEXT",
            "ALTER TABLE collections ADD COLUMN updated_at TEXT",
            "ALTER TABLE item_collections ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'synced'",
            "CREATE TABLE IF NOT EXISTS local_hidden_items (
                item_id TEXT PRIMARY KEY,
                hidden_at TEXT NOT NULL
            )",
            // Sync engine polls pending counts/lists frequently.
            "CREATE INDEX IF NOT EXISTS idx_items_sync_status ON items(sync_status)",
        ];
        for patch in patches {
            let _ = conn.execute(patch, []);
        }
        Ok(())
    }

    fn seed_defaults(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM collections", [], |r| r.get(0))?;
        if count == 0 {
            let now = Utc::now().to_rfc3339();
            for (name, color) in [
                ("Work", "#6366f1"),
                ("Personal", "#22c55e"),
                ("Research", "#f59e0b"),
            ] {
                conn.execute(
                    "INSERT INTO collections (id, name, color, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![Uuid::new_v4().to_string(), name, color, 0, now],
                )?;
            }
        }
        drop(conn);
        if self.get_setting(super::models::SETTING_HISTORY_RETENTION)?.is_none() {
            self.set_setting(
                super::models::SETTING_HISTORY_RETENTION,
                &super::models::DEFAULT_HISTORY_RETENTION_DAYS.to_string(),
            )?;
        }
        if self.get_setting(super::models::SETTING_THEME_PREFERENCE)?.is_none() {
            self.set_setting(
                super::models::SETTING_THEME_PREFERENCE,
                super::models::DEFAULT_THEME_PREFERENCE,
            )?;
        }
        Ok(())
    }

    pub fn blobs_dir(&self) -> &Path {
        &self.blobs_dir
    }

    pub fn ensure_device(&self) -> Result<String, rusqlite::Error> {
        if let Some(id) = self.get_setting(super::models::SETTING_LOCAL_DEVICE_ID)? {
            if !id.is_empty() {
                let conn = self.conn.lock();
                let exists: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM devices WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )?;
                if exists > 0 {
                    conn.execute(
                        "UPDATE devices SET is_current = CASE WHEN id = ?1 THEN 1 ELSE 0 END",
                        params![id],
                    )?;
                    return Ok(id);
                }
            }
        }

        self.rotate_local_device()
    }

    /// New local device identity (e.g. after switching cloud accounts on this machine).
    pub fn rotate_local_device(&self) -> Result<String, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let name = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "My Device".to_string());
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else {
            "windows"
        };
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock();
        conn.execute("UPDATE devices SET is_current = 0", [])?;
        conn.execute(
            "INSERT INTO devices (id, name, platform, last_seen_at, is_current, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            params![id, name, platform, now, now],
        )?;
        drop(conn);

        self.set_setting(super::models::SETTING_LOCAL_DEVICE_ID, &id)?;
        self.set_setting(super::models::SETTING_LOCAL_DEVICE_NAME, &name)?;
        Ok(id)
    }

    pub fn get_device_name(&self, device_id: &str) -> Result<String, rusqlite::Error> {
        self.conn.lock().query_row(
            "SELECT name FROM devices WHERE id = ?1",
            params![device_id],
            |r| r.get(0),
        )
    }

    pub fn touch_device(&self, device_id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "UPDATE devices SET last_seen_at = ?1 WHERE id = ?2",
            params![now, device_id],
        )?;
        Ok(())
    }

    pub fn insert_item(
        &self,
        device_id: &str,
        content_type: &str,
        plain_text: Option<String>,
        url: Option<String>,
        blob_path: Option<String>,
        blob_size: Option<i64>,
        thumbnail_path: Option<String>,
        content_hash: &str,
    ) -> Result<ItemRecord, rusqlite::Error> {
        // Dedupe: same hash within 5 minutes on any device
        let mut conn = self.conn.lock();
        let recent: Option<String> = conn
            .query_row(
                "SELECT id FROM items WHERE content_hash = ?1
                 AND deleted_at IS NULL AND datetime(created_at) > datetime('now', '-5 minutes')
                 LIMIT 1",
                params![content_hash],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = recent {
            drop(conn);
            return self.get_item(&id);
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let preview = plain_text.as_deref().map(truncate_preview);
        let char_count = plain_text.as_ref().map(|t| t.chars().count() as i64);
        let (url_domain, url_title) = url.as_ref().map(|u| extract_url_meta(u)).unwrap_or((None, None));
        let (code_language, line_count) = if content_type == "code" {
            detect_code(plain_text.as_deref())
        } else {
            (None, None)
        };
        let display_title: Option<String> = plain_text
            .as_ref()
            .map(|t| t.lines().next().unwrap_or(t).chars().take(80).collect());

        // Item row, search index, and sync queue must land atomically — a
        // crash between them leaves ghost/missing search results.
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO items (
                id, kind, content_type, display_title, preview_text, char_count,
                url, url_title, url_domain, code_language, line_count,
                blob_path, blob_size, thumbnail_path, content_hash, plain_text,
                source_device_id, sync_status, created_at, updated_at
            ) VALUES (?1,'history',?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'pending',?17,?17)",
            params![
                id,
                content_type,
                display_title,
                preview,
                char_count,
                url,
                url_title,
                url_domain,
                code_language,
                line_count,
                blob_path,
                blob_size,
                thumbnail_path,
                content_hash,
                plain_text,
                device_id,
                now,
            ],
        )?;

        let tags_str = String::new();
        tx.execute(
            "INSERT INTO items_fts (id, display_title, preview_text, url_domain, tags, trigger)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                display_title,
                preview,
                url_domain,
                tags_str,
                Option::<String>::None,
            ],
        )?;

        self.enqueue_sync(&tx, "create", "item", &id)?;
        tx.commit()?;

        drop(conn);
        self.get_item(&id)
    }

    fn enqueue_sync(
        &self,
        conn: &Connection,
        op: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sync_queue (id, op, entity_type, entity_id, payload, status, created_at)
             VALUES (?1, ?2, ?3, ?4, '{}', 'pending', ?5)",
            params![Uuid::new_v4().to_string(), op, entity_type, entity_id, now],
        )?;
        Ok(())
    }

    pub fn get_item(&self, id: &str) -> Result<ItemRecord, rusqlite::Error> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT i.id, i.kind, i.content_type, i.display_title, i.preview_text, i.char_count,
                    i.url, i.url_title, i.url_domain, i.code_language, i.line_count,
                    i.blob_path, i.blob_size, i.thumbnail_path, i.content_hash, i.plain_text,
                    i.trigger, i.source_device_id, d.name, i.is_pinned, i.is_favorited,
                    i.sync_status, i.created_at, i.updated_at, i.deleted_at
             FROM items i
             LEFT JOIN devices d ON d.id = i.source_device_id
             WHERE i.id = ?1 AND i.deleted_at IS NULL",
            params![id],
            map_item_row,
        )
    }

    pub fn list_items(&self, limit: i64) -> Result<Vec<ItemRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.kind, i.content_type, i.display_title, i.preview_text, i.char_count,
                    i.url, i.url_title, i.url_domain, i.code_language, i.line_count,
                    i.blob_path, i.blob_size, i.thumbnail_path, i.content_hash, i.plain_text,
                    i.trigger, i.source_device_id, d.name, i.is_pinned, i.is_favorited,
                    i.sync_status, i.created_at, i.updated_at, i.deleted_at
             FROM items i
             LEFT JOIN devices d ON d.id = i.source_device_id
             WHERE i.deleted_at IS NULL
             ORDER BY i.is_pinned DESC, i.created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], map_item_row)?;
        rows.collect()
    }

    pub fn search(&self, filters: &SearchFiltersDto) -> Result<Vec<ItemRecord>, rusqlite::Error> {
        let parsed = crate::search::parse_query(&filters.query);
        let conn = self.conn.lock();

        let mut sql = String::from(
            "SELECT i.id, i.kind, i.content_type, i.display_title, i.preview_text, i.char_count,
                    i.url, i.url_title, i.url_domain, i.code_language, i.line_count,
                    i.blob_path, i.blob_size, i.thumbnail_path, i.content_hash, i.plain_text,
                    i.trigger, i.source_device_id, d.name, i.is_pinned, i.is_favorited,
                    i.sync_status, i.created_at, i.updated_at, i.deleted_at
             FROM items i
             LEFT JOIN devices d ON d.id = i.source_device_id
             WHERE i.deleted_at IS NULL",
        );

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(ref text) = parsed.text {
            if !text.is_empty() {
                sql.push_str(" AND i.id IN (SELECT id FROM items_fts WHERE items_fts MATCH ?)");
                params_vec.push(Box::new(format!("{}*", text.replace('"', ""))));
            }
        }

        if let Some(device) = filters.device.as_ref().or(parsed.device.as_ref()) {
            sql.push_str(" AND d.platform = ?");
            params_vec.push(Box::new(device.clone()));
        }
        if let Some(ct) = filters.content_type.as_ref().or(parsed.content_type.as_ref()) {
            sql.push_str(" AND i.content_type = ?");
            params_vec.push(Box::new(ct.clone()));
        }
        if filters.is_pinned.unwrap_or(false) || parsed.is_pinned {
            sql.push_str(" AND i.is_pinned = 1");
        }
        if filters.is_favorite.unwrap_or(false) || parsed.is_favorite {
            sql.push_str(" AND i.is_favorited = 1");
        }
        if filters.is_snippet.unwrap_or(false) || parsed.is_snippet {
            sql.push_str(" AND i.kind = 'snippet'");
        }
        if filters.date_today.unwrap_or(false) || parsed.date_today {
            sql.push_str(" AND date(i.created_at) = date('now', 'localtime')");
        }
        if let Some(tag) = parsed.tag.as_ref() {
            sql.push_str(
                " AND i.id IN (SELECT item_id FROM item_tags it JOIN tags t ON t.id = it.tag_id WHERE t.name = ?)",
            );
            params_vec.push(Box::new(tag.clone()));
        }
        if let Some(ref collection) = filters.collection {
            sql.push_str(
                " AND i.id IN (SELECT item_id FROM item_collections WHERE collection_id = ?)",
            );
            params_vec.push(Box::new(collection.clone()));
        }
        if filters.in_collection.unwrap_or(false) {
            sql.push_str(" AND i.id IN (SELECT item_id FROM item_collections)");
        }

        sql.push_str(" ORDER BY i.is_pinned DESC, i.created_at DESC LIMIT 50");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), map_item_row)?;
        rows.collect()
    }

    pub fn get_timeline(&self) -> Result<Vec<TimelineSectionDto>, rusqlite::Error> {
        let items = self.list_items(500)?;
        Ok(timeline::build_timeline(&items))
    }

    pub fn get_tab_timeline(
        &self,
        tab: &str,
        collection_id: Option<&str>,
    ) -> Result<Vec<TimelineSectionDto>, rusqlite::Error> {
        let items = self.list_items_by_tab(tab, collection_id, 500)?;
        if tab == "history" {
            return Ok(timeline::build_history_timeline(&items));
        }
        let label = match tab {
            "pinned" => "Pinned",
            "favorites" => "Favorites",
            "collections" => "Collections",
            "snippets" => "Snippets",
            _ => "Items",
        };
        Ok(vec![TimelineSectionDto {
            bucket: tab.to_string(),
            label: label.to_string(),
            items: items.iter().map(item_to_preview).collect(),
        }])
    }

    pub fn list_items_by_tab(
        &self,
        tab: &str,
        collection_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ItemRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let base = "SELECT i.id, i.kind, i.content_type, i.display_title, i.preview_text, i.char_count,
                    i.url, i.url_title, i.url_domain, i.code_language, i.line_count,
                    i.blob_path, i.blob_size, i.thumbnail_path, i.content_hash, i.plain_text,
                    i.trigger, i.source_device_id, d.name, i.is_pinned, i.is_favorited,
                    i.sync_status, i.created_at, i.updated_at, i.deleted_at
             FROM items i
             LEFT JOIN devices d ON d.id = i.source_device_id";

        let (where_clause, mut params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            match tab {
                "history" => (
                    "WHERE i.deleted_at IS NULL AND i.kind = 'history' AND i.is_pinned = 0"
                        .to_string(),
                    vec![],
                ),
                "pinned" => (
                    "WHERE i.deleted_at IS NULL AND i.is_pinned = 1".to_string(),
                    vec![],
                ),
                "favorites" => (
                    "WHERE i.deleted_at IS NULL AND i.is_favorited = 1".to_string(),
                    vec![],
                ),
                "collections" => {
                    if let Some(cid) = collection_id {
                        (
                            "WHERE i.deleted_at IS NULL AND i.id IN (SELECT item_id FROM item_collections WHERE collection_id = ?1)".to_string(),
                            vec![Box::new(cid.to_string())],
                        )
                    } else {
                        (
                            "WHERE i.deleted_at IS NULL AND i.id IN (SELECT item_id FROM item_collections)".to_string(),
                            vec![],
                        )
                    }
                }
                "snippets" => (
                    "WHERE i.deleted_at IS NULL AND i.kind = 'snippet'".to_string(),
                    vec![],
                ),
                _ => ("WHERE i.deleted_at IS NULL".to_string(), vec![]),
            };

        let sql = format!(
            "{base} {where_clause} ORDER BY i.is_pinned DESC, i.created_at DESC LIMIT ?",
        );
        params_vec.push(Box::new(limit));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), map_item_row)?;
        rows.collect()
    }

    pub fn content_hash_exists(&self, content_hash: &str) -> Result<bool, rusqlite::Error> {
        let count: i64 = self.conn.lock().query_row(
            "SELECT COUNT(*) FROM items WHERE content_hash = ?1 AND deleted_at IS NULL",
            params![content_hash],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn content_hash_synced_exists(&self, content_hash: &str) -> Result<bool, rusqlite::Error> {
        let count: i64 = self.conn.lock().query_row(
            "SELECT COUNT(*) FROM items WHERE content_hash = ?1 AND deleted_at IS NULL AND sync_status = 'synced'",
            params![content_hash],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn recent_plain_text_exists(&self, text: &str, limit: i64) -> Result<bool, rusqlite::Error> {
        let count: i64 = self.conn.lock().query_row(
            "SELECT COUNT(*) FROM (
                SELECT plain_text FROM items
                WHERE deleted_at IS NULL AND plain_text = ?1
                ORDER BY created_at DESC LIMIT ?2
             )",
            params![text, limit],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn toggle_pin(&self, id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE items SET is_pinned = CASE is_pinned WHEN 1 THEN 0 ELSE 1 END, updated_at = ?1, sync_status = 'pending' WHERE id = ?2",
            params![now, id],
        )?;
        self.enqueue_sync(&tx, "update", "item", id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn toggle_favorite(&self, id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE items SET is_favorited = CASE is_favorited WHEN 1 THEN 0 ELSE 1 END, updated_at = ?1, sync_status = 'pending' WHERE id = ?2",
            params![now, id],
        )?;
        self.enqueue_sync(&tx, "update", "item", id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_item(&self, id: &str) -> Result<(), rusqlite::Error> {
        self.soft_delete_item(id, true)
    }

    fn soft_delete_item(&self, id: &str, remove_blob: bool) -> Result<(), rusqlite::Error> {
        self.soft_delete_item_with_sync(id, remove_blob, true)
    }

    fn soft_delete_item_with_sync(
        &self,
        id: &str,
        remove_blob: bool,
        sync_to_cloud: bool,
    ) -> Result<(), rusqlite::Error> {
        let blob_path: Option<String> = if remove_blob {
            self.conn.lock().query_row(
                "SELECT blob_path FROM items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            ).optional()?.flatten()
        } else {
            None
        };

        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        if sync_to_cloud {
            tx.execute(
                "UPDATE items SET deleted_at = ?1, updated_at = ?1, sync_status = 'pending' WHERE id = ?2 AND deleted_at IS NULL",
                params![now, id],
            )?;
            self.enqueue_sync(&tx, "delete", "item", id)?;
        } else {
            let sync_status: String = tx
                .query_row(
                    "SELECT sync_status FROM items WHERE id = ?1 AND deleted_at IS NULL",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_else(|| "synced".to_string());
            tx.execute(
                "UPDATE items SET deleted_at = ?1, updated_at = ?1, sync_status = 'synced' WHERE id = ?2 AND deleted_at IS NULL",
                params![now, id],
            )?;
            if sync_status == "synced" {
                tx.execute(
                    "INSERT OR IGNORE INTO local_hidden_items (item_id, hidden_at) VALUES (?1, ?2)",
                    params![id, now],
                )?;
            }
        }
        tx.execute("DELETE FROM items_fts WHERE id = ?1", params![id])?;
        tx.commit()?;
        drop(conn);

        if let Some(path) = blob_path {
            std::fs::remove_file(path).ok();
        }
        Ok(())
    }

    fn is_locally_hidden(&self, item_id: &str) -> Result<bool, rusqlite::Error> {
        let hidden: Option<i64> = self.conn.lock().query_row(
            "SELECT 1 FROM local_hidden_items WHERE item_id = ?1",
            params![item_id],
            |r| r.get(0),
        ).optional()?;
        Ok(hidden.is_some())
    }

    const CLEARABLE_HISTORY_BASE: &'static str = "SELECT i.id FROM items i
             WHERE i.deleted_at IS NULL
               AND i.kind = 'history'
               AND i.is_pinned = 0
               AND i.is_favorited = 0
               AND i.id NOT IN (SELECT item_id FROM item_collections)";

    fn count_clearable_history(
        &self,
        mode: ClearHistoryMode,
        retention_days: i64,
    ) -> Result<u32, rusqlite::Error> {
        let conn = self.conn.lock();
        let count: i64 = match mode {
            ClearHistoryMode::All => conn.query_row(
                &format!("SELECT COUNT(*) FROM ({}) AS clearable", Self::CLEARABLE_HISTORY_BASE),
                [],
                |r| r.get(0),
            )?,
            ClearHistoryMode::Expired if retention_days <= 0 => 0,
            ClearHistoryMode::Expired => {
                let cutoff = format!("-{retention_days} days");
                conn.query_row(
                    &format!(
                        "SELECT COUNT(*) FROM ({base} AND datetime(i.created_at) < datetime('now', ?1)) AS clearable",
                        base = Self::CLEARABLE_HISTORY_BASE
                    ),
                    params![cutoff],
                    |r| r.get(0),
                )?
            }
        };
        Ok(count as u32)
    }

    fn list_clearable_history_ids(
        &self,
        mode: ClearHistoryMode,
        retention_days: i64,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock();
        match mode {
            ClearHistoryMode::All => {
                let mut stmt = conn.prepare(Self::CLEARABLE_HISTORY_BASE)?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect()
            }
            ClearHistoryMode::Expired if retention_days <= 0 => Ok(vec![]),
            ClearHistoryMode::Expired => {
                let cutoff = format!("-{retention_days} days");
                let sql = format!(
                    "{} AND datetime(i.created_at) < datetime('now', ?1)",
                    Self::CLEARABLE_HISTORY_BASE
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(params![cutoff], |row| row.get(0))?;
                rows.collect()
            }
        }
    }

    pub fn preview_clear_history(&self) -> Result<ClearHistoryPreviewDto, rusqlite::Error> {
        let retention_days = self.get_history_retention_days()?;
        let expired_count = if retention_days > 0 {
            self.count_clearable_history(ClearHistoryMode::Expired, retention_days)?
        } else {
            0
        };
        let all_count = self.count_clearable_history(ClearHistoryMode::All, retention_days)?;
        Ok(ClearHistoryPreviewDto {
            expired_count,
            all_count,
            retention_days,
        })
    }

    pub fn clear_history(
        &self,
        scope: ClearHistoryScope,
        mode: ClearHistoryMode,
    ) -> Result<ClearHistoryResultDto, rusqlite::Error> {
        let retention_days = self.get_history_retention_days()?;
        if mode == ClearHistoryMode::Expired && retention_days <= 0 {
            return Ok(ClearHistoryResultDto {
                cleared: 0,
                scope: scope_label(scope).to_string(),
                mode: mode_label(mode).to_string(),
            });
        }

        let ids = self.list_clearable_history_ids(mode, retention_days)?;
        let sync_to_cloud = scope == ClearHistoryScope::Everywhere;
        let mut cleared = 0u32;

        for id in ids {
            if self
                .soft_delete_item_with_sync(&id, true, sync_to_cloud)
                .is_ok()
            {
                if sync_to_cloud {
                    let _ = self.conn.lock().execute(
                        "DELETE FROM local_hidden_items WHERE item_id = ?1",
                        params![id],
                    );
                }
                cleared += 1;
            }
        }

        Ok(ClearHistoryResultDto {
            cleared,
            scope: scope_label(scope).to_string(),
            mode: mode_label(mode).to_string(),
        })
    }

    pub fn purge_expired_history(&self) -> Result<u32, rusqlite::Error> {
        let result = self.clear_history(ClearHistoryScope::Everywhere, ClearHistoryMode::Expired)?;
        Ok(result.cleared)
    }

    pub fn apply_remote_deletion(&self, id: &str, deleted_at: &str) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE items SET deleted_at = ?1, sync_status = 'synced' WHERE id = ?2 AND deleted_at IS NULL",
            params![deleted_at, id],
        )?;
        tx.execute("DELETE FROM items_fts WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_history_retention_days(&self) -> Result<i64, rusqlite::Error> {
        match self.get_setting(super::models::SETTING_HISTORY_RETENTION)? {
            Some(v) => Ok(v.parse().unwrap_or(super::models::DEFAULT_HISTORY_RETENTION_DAYS)),
            None => Ok(super::models::DEFAULT_HISTORY_RETENTION_DAYS),
        }
    }

    pub fn set_history_retention_days(&self, days: i64) -> Result<(), rusqlite::Error> {
        if ![0, 30, 60, 90].contains(&days) {
            return Err(rusqlite::Error::InvalidParameterName(
                "history_retention_days".into(),
            ));
        }
        self.set_setting(super::models::SETTING_HISTORY_RETENTION, &days.to_string())
    }

    pub fn get_app_settings(&self) -> Result<super::models::AppSettingsDto, rusqlite::Error> {
        Ok(super::models::AppSettingsDto {
            history_retention_days: self.get_history_retention_days()?,
            clipboard_paused: self.get_clipboard_paused()?,
            theme_preference: self.get_theme_preference()?,
            launch_at_login: false,
        })
    }

    pub fn get_theme_preference(&self) -> Result<String, rusqlite::Error> {
        Ok(self
            .get_setting(super::models::SETTING_THEME_PREFERENCE)?
            .unwrap_or_else(|| super::models::DEFAULT_THEME_PREFERENCE.to_string()))
    }

    pub fn set_theme_preference(&self, preference: &str) -> Result<(), rusqlite::Error> {
        if !["system", "light", "dark"].contains(&preference) {
            return Err(rusqlite::Error::InvalidParameterName(
                "theme_preference".into(),
            ));
        }
        self.set_setting(super::models::SETTING_THEME_PREFERENCE, preference)
    }

    pub fn get_clipboard_paused(&self) -> Result<bool, rusqlite::Error> {
        Ok(self
            .get_setting(super::models::SETTING_CLIPBOARD_PAUSED)?
            .is_some_and(|v| v == "1"))
    }

    pub fn set_clipboard_paused(&self, paused: bool) -> Result<(), rusqlite::Error> {
        self.set_setting(
            super::models::SETTING_CLIPBOARD_PAUSED,
            if paused { "1" } else { "0" },
        )
    }

    pub fn rename_item(&self, id: &str, title: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE items SET display_title = ?1, updated_at = ?2, sync_status = 'pending' WHERE id = ?3",
            params![title, now, id],
        )?;
        tx.execute(
            "UPDATE items_fts SET display_title = ?1 WHERE id = ?2",
            params![title, id],
        )?;
        self.enqueue_sync(&tx, "update", "item", id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_collections(&self) -> Result<Vec<CollectionDto>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.name, c.color, c.icon,
                    (SELECT COUNT(*) FROM item_collections ic WHERE ic.collection_id = c.id) as cnt
             FROM collections c
             WHERE c.deleted_at IS NULL
             ORDER BY c.sort_order, c.name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CollectionDto {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                icon: row.get(3)?,
                item_count: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn create_snippet(
        &self,
        device_id: &str,
        title: &str,
        text: &str,
        trigger: Option<&str>,
    ) -> Result<ItemRecord, rusqlite::Error> {
        let trimmed_title = title.trim();
        let trimmed_text = text.trim();
        if trimmed_title.is_empty() || trimmed_text.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName("title/text".into()));
        }

        let content_hash =
            crate::clipboard::hash_content("text", Some(trimmed_text), None);
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let preview = truncate_preview(trimmed_text);
        let char_count = trimmed_text.chars().count() as i64;
        let trigger_val = trigger
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from);

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO items (
                id, kind, content_type, display_title, preview_text, char_count,
                url, url_title, url_domain, code_language, line_count,
                blob_path, blob_size, thumbnail_path, content_hash, plain_text,
                trigger, source_device_id, sync_status, created_at, updated_at
            ) VALUES (?1,'snippet','text',?2,?3,?4,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,?5,?6,?7,?8,'pending',?9,?9)",
            params![
                id,
                trimmed_title,
                preview,
                char_count,
                content_hash,
                trimmed_text,
                trigger_val,
                device_id,
                now,
            ],
        )?;

        tx.execute(
            "INSERT INTO items_fts (id, display_title, preview_text, url_domain, tags, trigger)
             VALUES (?1, ?2, ?3, NULL, '', ?4)",
            params![id, trimmed_title, preview, trigger_val.as_deref()],
        )?;

        self.enqueue_sync(&tx, "create", "item", &id)?;
        tx.commit()?;
        drop(conn);
        self.get_item(&id)
    }

    pub fn update_snippet(
        &self,
        id: &str,
        title: &str,
        text: &str,
        trigger: Option<&str>,
    ) -> Result<ItemRecord, rusqlite::Error> {
        let trimmed_title = title.trim();
        let trimmed_text = text.trim();
        if trimmed_title.is_empty() || trimmed_text.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName("title/text".into()));
        }

        let item = self.get_item(id)?;
        if item.kind != "snippet" {
            return Err(rusqlite::Error::InvalidParameterName("not a snippet".into()));
        }
        if item.deleted_at.is_some() {
            return Err(rusqlite::Error::InvalidParameterName("deleted".into()));
        }

        let content_hash =
            crate::clipboard::hash_content("text", Some(trimmed_text), None);
        let now = Utc::now().to_rfc3339();
        let preview = truncate_preview(trimmed_text);
        let char_count = trimmed_text.chars().count() as i64;
        let trigger_val = trigger
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from);

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE items SET
                display_title = ?1, plain_text = ?2, preview_text = ?3, char_count = ?4,
                content_hash = ?5, trigger = ?6, updated_at = ?7, sync_status = 'pending'
             WHERE id = ?8",
            params![
                trimmed_title,
                trimmed_text,
                preview,
                char_count,
                content_hash,
                trigger_val,
                now,
                id,
            ],
        )?;
        tx.execute(
            "UPDATE items_fts SET display_title = ?1, preview_text = ?2, trigger = ?3 WHERE id = ?4",
            params![trimmed_title, preview, trigger_val.as_deref(), id],
        )?;
        self.enqueue_sync(&tx, "update", "item", id)?;
        tx.commit()?;
        drop(conn);
        self.get_item(id)
    }

    pub fn save_item_as_snippet(&self, id: &str) -> Result<ItemRecord, rusqlite::Error> {
        let item = self.get_item(id)?;
        if item.kind == "snippet" {
            return Err(rusqlite::Error::InvalidParameterName("already snippet".into()));
        }
        if item.deleted_at.is_some() {
            return Err(rusqlite::Error::InvalidParameterName("deleted".into()));
        }

        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE items SET kind = 'snippet', updated_at = ?1, sync_status = 'pending' WHERE id = ?2",
            params![now, id],
        )?;
        self.enqueue_sync(&conn, "update", "item", id)?;
        drop(conn);
        self.get_item(id)
    }

    pub fn create_collection(&self, name: &str, color: &str) -> Result<CollectionDto, rusqlite::Error> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName("name".into()));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let max_order: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM collections",
                [],
                |r| r.get(0),
            )
            .unwrap_or(-1);
        tx.execute(
            "INSERT INTO collections (id, name, color, sort_order, created_at, updated_at, sync_status) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 'pending')",
            params![id, trimmed, color, max_order + 1, now],
        )?;
        self.enqueue_sync(&tx, "create", "collection", &id)?;
        tx.commit()?;
        drop(conn);
        self.get_collection(&id)
    }

    pub fn get_collection(&self, id: &str) -> Result<CollectionDto, rusqlite::Error> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT c.id, c.name, c.color, c.icon,
                    (SELECT COUNT(*) FROM item_collections ic WHERE ic.collection_id = c.id) as cnt
             FROM collections c WHERE c.id = ?1 AND c.deleted_at IS NULL",
            params![id],
            |row| {
                Ok(CollectionDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    icon: row.get(3)?,
                    item_count: row.get(4)?,
                })
            },
        )
    }

    pub fn update_collection(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
    ) -> Result<CollectionDto, rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        if let Some(n) = name {
            if n.trim().is_empty() {
                return Err(rusqlite::Error::InvalidParameterName("name".into()));
            }
            tx.execute(
                "UPDATE collections SET name = ?1, updated_at = ?2, sync_status = 'pending' WHERE id = ?3 AND deleted_at IS NULL",
                params![n.trim(), now, id],
            )?;
        }
        if let Some(c) = color {
            tx.execute(
                "UPDATE collections SET color = ?1, updated_at = ?2, sync_status = 'pending' WHERE id = ?3 AND deleted_at IS NULL",
                params![c, now, id],
            )?;
        }
        self.enqueue_sync(&tx, "update", "collection", id)?;
        tx.commit()?;
        drop(conn);
        self.get_collection(id)
    }

    pub fn delete_collection(&self, id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM item_collections WHERE collection_id = ?1",
            params![id],
        )?;
        let changed = tx.execute(
            "UPDATE collections SET deleted_at = ?1, updated_at = ?1, sync_status = 'pending' WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        self.enqueue_sync(&tx, "delete", "collection", id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn add_item_to_collection(&self, item_id: &str, collection_id: &str) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let exists: i64 = tx.query_row(
            "SELECT COUNT(*) FROM item_collections WHERE item_id = ?1 AND collection_id = ?2",
            params![item_id, collection_id],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Ok(());
        }
        // Ensure the parent collection is queued for cloud push before the link.
        let collection_pending: i64 = tx.query_row(
            "SELECT COUNT(*) FROM collections WHERE id = ?1 AND sync_status = 'pending' AND deleted_at IS NULL",
            params![collection_id],
            |r| r.get(0),
        )?;
        if collection_pending > 0 {
            self.enqueue_sync(&tx, "create", "collection", collection_id)?;
        }
        tx.execute(
            "INSERT INTO item_collections (item_id, collection_id, sync_status) VALUES (?1, ?2, 'pending')",
            params![item_id, collection_id],
        )?;
        let key = format!("{item_id}:{collection_id}");
        self.enqueue_sync(&tx, "create", "item_collection", &key)?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_item_from_collection(
        &self,
        item_id: &str,
        collection_id: &str,
    ) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "DELETE FROM item_collections WHERE item_id = ?1 AND collection_id = ?2",
            params![item_id, collection_id],
        )?;
        if changed == 0 {
            return Ok(());
        }
        let key = format!("{item_id}:{collection_id}");
        self.enqueue_sync(&tx, "delete", "item_collection", &key)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_item_collection_ids(&self, item_id: &str) -> Result<Vec<String>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT collection_id FROM item_collections WHERE item_id = ?1",
        )?;
        let rows = stmt.query_map(params![item_id], |row| row.get(0))?;
        rows.collect()
    }

    pub fn get_devices(&self) -> Result<Vec<DeviceDto>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, platform, last_seen_at, is_current FROM devices ORDER BY is_current DESC, name",
        )?;
        let now = Utc::now();
        let rows = stmt.query_map([], |row| {
            let last_seen: Option<String> = row.get(3)?;
            let is_online = last_seen
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|t| now.signed_duration_since(t.with_timezone(&Utc)) < Duration::minutes(2))
                .unwrap_or(false);
            Ok(DeviceDto {
                id: row.get(0)?,
                name: row.get(1)?,
                platform: row.get(2)?,
                last_seen_at: last_seen,
                is_current: row.get::<_, i64>(4)? == 1,
                is_online,
            })
        })?;
        rows.collect()
    }

    pub fn pending_sync_count(&self) -> Result<i64, rusqlite::Error> {
        self.conn.lock().query_row(
            "SELECT
                (SELECT COUNT(*) FROM items WHERE sync_status = 'pending') +
                (SELECT COUNT(*) FROM collections WHERE sync_status = 'pending') +
                (SELECT COUNT(*) FROM item_collections WHERE sync_status = 'pending')",
            [],
            |r| r.get(0),
        )
    }

    pub fn clear_pending_sync_queue(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock();
        let cleared = conn.execute("DELETE FROM sync_queue WHERE status = 'pending'", [])?;
        Ok(cleared as i64)
    }

    /// Flag every item (including soft-deleted ones — their cloud rows also
    /// hold plaintext) for re-push. Used by the one-time E2E encryption
    /// backfill so pre-encryption cloud rows get overwritten as ciphertext.
    pub fn mark_all_items_pending(&self) -> Result<i64, rusqlite::Error> {
        let changed = self
            .conn
            .lock()
            .execute("UPDATE items SET sync_status = 'pending'", [])?;
        Ok(changed as i64)
    }

    /// Remove completed queue rows so the table doesn't grow without bound.
    pub fn prune_synced_queue_rows(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock();
        let pruned = conn.execute("DELETE FROM sync_queue WHERE status = 'synced'", [])?;
        Ok(pruned as i64)
    }

    pub fn mark_synced(&self, entity_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE items SET sync_status = 'synced' WHERE id = ?1",
            params![entity_id],
        )?;
        conn.execute(
            "UPDATE sync_queue SET status = 'synced' WHERE entity_id = ?1 AND status = 'pending'",
            params![entity_id],
        )?;
        Ok(())
    }

    pub fn mark_collection_synced(&self, entity_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE collections SET sync_status = 'synced' WHERE id = ?1",
            params![entity_id],
        )?;
        conn.execute(
            "DELETE FROM collections WHERE id = ?1 AND deleted_at IS NOT NULL",
            params![entity_id],
        )?;
        conn.execute(
            "UPDATE sync_queue SET status = 'synced' WHERE entity_id = ?1 AND status = 'pending'",
            params![entity_id],
        )?;
        Ok(())
    }

    pub fn mark_item_collection_synced(&self, item_id: &str, collection_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let key = format!("{item_id}:{collection_id}");
        conn.execute(
            "UPDATE item_collections SET sync_status = 'synced'
             WHERE item_id = ?1 AND collection_id = ?2",
            params![item_id, collection_id],
        )?;
        conn.execute(
            "UPDATE sync_queue SET status = 'synced' WHERE entity_id = ?1 AND status = 'pending'",
            params![key],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.conn.lock().query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        ).optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().execute(
            "DELETE FROM settings WHERE key = ?1",
            params![key],
        )?;
        Ok(())
    }

    pub fn list_pending_sync_items(&self) -> Result<Vec<ItemRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.kind, i.content_type, i.display_title, i.preview_text, i.char_count,
                    i.url, i.url_title, i.url_domain, i.code_language, i.line_count,
                    i.blob_path, i.blob_size, i.thumbnail_path, i.content_hash, i.plain_text,
                    i.trigger, i.source_device_id, d.name, i.is_pinned, i.is_favorited,
                    i.sync_status, i.created_at, i.updated_at, i.deleted_at
             FROM items i
             LEFT JOIN devices d ON d.id = i.source_device_id
             WHERE i.sync_status = 'pending'
             ORDER BY i.created_at ASC
             LIMIT 50",
        )?;
        let rows = stmt.query_map([], map_item_row)?;
        rows.collect()
    }

    pub fn item_exists(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let count: i64 = self.conn.lock().query_row(
            "SELECT COUNT(*) FROM items WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn list_pending_sync_collections(&self) -> Result<Vec<CollectionRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, icon, sort_order, sync_status, created_at, updated_at, deleted_at
             FROM collections WHERE sync_status = 'pending' ORDER BY created_at ASC LIMIT 50",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CollectionRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                icon: row.get(3)?,
                sort_order: row.get(4)?,
                sync_status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_pending_sync_item_collections(&self) -> Result<Vec<ItemCollectionRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT ic.item_id, ic.collection_id
             FROM item_collections ic
             INNER JOIN collections c ON c.id = ic.collection_id AND c.deleted_at IS NULL
             INNER JOIN items i ON i.id = ic.item_id AND i.deleted_at IS NULL
             WHERE ic.sync_status = 'pending'
               AND c.sync_status = 'synced'
               AND i.sync_status = 'synced'
             LIMIT 50",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ItemCollectionRecord {
                item_id: row.get(0)?,
                collection_id: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_pending_item_collection_deletes(&self) -> Result<Vec<ItemCollectionRecord>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT entity_id FROM sync_queue
             WHERE entity_type = 'item_collection' AND op = 'delete' AND status = 'pending'
             LIMIT 50",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let (item_id, collection_id) = key
                    .split_once(':')
                    .ok_or_else(|| rusqlite::Error::InvalidParameterName(key.clone()))?;
                Ok(ItemCollectionRecord {
                    item_id: item_id.to_string(),
                    collection_id: collection_id.to_string(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn upsert_remote_collection(
        &self,
        collection: &crate::sync::client::CloudCollection,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO collections (id, name, color, icon, sort_order, created_at, updated_at, sync_status, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 'synced', NULL)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                color = excluded.color,
                icon = excluded.icon,
                sort_order = excluded.sort_order,
                updated_at = excluded.updated_at,
                sync_status = 'synced',
                deleted_at = NULL",
            params![
                collection.id,
                collection.name,
                collection.color,
                collection.icon,
                collection.sort_order,
                collection.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_remote_collection(&self, id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM item_collections WHERE collection_id = ?1", params![id])?;
        conn.execute("DELETE FROM collections WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn upsert_remote_item_collection(
        &self,
        link: &crate::sync::client::CloudItemCollection,
    ) -> Result<(), rusqlite::Error> {
        self.ensure_collection_exists(&link.collection_id)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO item_collections (item_id, collection_id, sync_status)
             VALUES (?1, ?2, 'synced')
             ON CONFLICT(item_id, collection_id) DO UPDATE SET sync_status = 'synced'",
            params![link.item_id, link.collection_id],
        )?;
        Ok(())
    }

    pub fn delete_remote_item_collection(&self, item_id: &str, collection_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM item_collections WHERE item_id = ?1 AND collection_id = ?2",
            params![item_id, collection_id],
        )?;
        Ok(())
    }

    pub fn upsert_remote_item(&self, item: &crate::sync::client::CloudItem) -> Result<(), rusqlite::Error> {
        if self.is_locally_hidden(&item.id)? {
            return Ok(());
        }

        if let Some(ref device_id) = item.source_device_id {
            self.ensure_device_exists(device_id)?;
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO items (
                id, kind, content_type, display_title, preview_text, char_count,
                url, url_title, url_domain, code_language, line_count,
                blob_path, blob_size, thumbnail_path, content_hash, plain_text,
                trigger, source_device_id, is_pinned, is_favorited, sync_status,
                created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,NULL,?14,?15,?16,?17,?18,?19,'synced',?20,?21)
            ON CONFLICT(id) DO UPDATE SET
                display_title = excluded.display_title,
                preview_text = excluded.preview_text,
                is_pinned = excluded.is_pinned,
                is_favorited = excluded.is_favorited,
                updated_at = excluded.updated_at,
                sync_status = 'synced'
            WHERE excluded.updated_at > items.updated_at
               OR items.sync_status != 'pending'",
            params![
                item.id,
                item.kind,
                item.content_type,
                item.display_title,
                item.preview_text,
                item.char_count,
                item.url,
                item.url_title,
                item.url_domain,
                item.code_language,
                item.line_count,
                // A blob_path from another device's local filesystem can
                // never be valid here — discard it even if an older client
                // version pushed one before this was fixed.
                Option::<String>::None,
                item.blob_size,
                item.content_hash,
                item.plain_text,
                item.trigger,
                item.source_device_id,
                i64::from(item.is_pinned),
                i64::from(item.is_favorited),
                item.created_at,
                item.updated_at,
            ],
        )?;

        // Rebuild the search index from the stored row (not the remote
        // payload) — the upsert above may have kept newer local values.
        tx.execute("DELETE FROM items_fts WHERE id = ?1", params![item.id])?;
        tx.execute(
            "INSERT INTO items_fts (id, display_title, preview_text, url_domain, tags, trigger)
             SELECT id, display_title, preview_text, url_domain, '', trigger
             FROM items WHERE id = ?1 AND deleted_at IS NULL",
            params![item.id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_remote_device(&self, device: &crate::sync::client::CloudDevice) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let is_current: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM devices WHERE id = ?1 AND is_current = 1",
                params![device.id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if is_current > 0 {
            return Ok(());
        }

        let created = device
            .last_seen_at
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        conn.execute(
            "INSERT INTO devices (id, name, platform, last_seen_at, is_current, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                platform = excluded.platform,
                last_seen_at = excluded.last_seen_at",
            params![
                device.id,
                device.name,
                device.platform,
                device.last_seen_at,
                created,
            ],
        )?;
        Ok(())
    }

    /// Remove local device rows that no longer exist in the cloud (e.g.
    /// duplicates merged server-side by `register_device`). Their items are
    /// reattributed to the current device first, so history keeps a valid
    /// source and the FK stays satisfied.
    pub fn prune_local_devices_not_in(
        &self,
        remote_ids: &[String],
        current_device_id: &str,
    ) -> Result<u32, rusqlite::Error> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let stale_ids: Vec<String> = {
            let mut stmt =
                tx.prepare("SELECT id FROM devices WHERE is_current = 0 AND id != ?1")?;
            let rows = stmt.query_map(params![current_device_id], |row| row.get(0))?;
            rows.collect::<Result<Vec<String>, _>>()?
                .into_iter()
                .filter(|id| !remote_ids.contains(id))
                .collect()
        };

        for id in &stale_ids {
            tx.execute(
                "UPDATE items SET source_device_id = ?1 WHERE source_device_id = ?2",
                params![current_device_id, id],
            )?;
            tx.execute("DELETE FROM devices WHERE id = ?1", params![id])?;
        }

        tx.commit()?;
        Ok(stale_ids.len() as u32)
    }

    fn ensure_device_exists(&self, device_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM devices WHERE id = ?1",
            params![device_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO devices (id, name, platform, last_seen_at, is_current, created_at)
                 VALUES (?1, 'Remote Device', 'unknown', NULL, 0, ?2)",
                params![device_id, now],
            )?;
        }
        Ok(())
    }

    fn ensure_collection_exists(&self, collection_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM collections WHERE id = ?1",
            params![collection_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO collections (id, name, color, sort_order, created_at, updated_at, sync_status)
                 VALUES (?1, 'Remote Collection', '#6366f1', 0, ?2, ?2, 'synced')",
                params![collection_id, now],
            )?;
        }
        Ok(())
    }
}

fn map_item_row(row: &rusqlite::Row<'_>) -> Result<ItemRecord, rusqlite::Error> {
    Ok(ItemRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        content_type: row.get(2)?,
        display_title: row.get(3)?,
        preview_text: row.get(4)?,
        char_count: row.get(5)?,
        url: row.get(6)?,
        url_title: row.get(7)?,
        url_domain: row.get(8)?,
        code_language: row.get(9)?,
        line_count: row.get(10)?,
        blob_path: row.get(11)?,
        blob_size: row.get(12)?,
        thumbnail_path: row.get(13)?,
        content_hash: row.get(14)?,
        plain_text: row.get(15)?,
        trigger: row.get(16)?,
        source_device_id: row.get(17)?,
        source_device_name: row.get(18)?,
        is_pinned: row.get::<_, i64>(19)? == 1,
        is_favorited: row.get::<_, i64>(20)? == 1,
        sync_status: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
        deleted_at: row.get(24)?,
    })
}

pub fn item_to_preview(item: &ItemRecord) -> PreviewCardDto {
    let kind = if item.kind == "snippet" {
        "snippet".to_string()
    } else {
        item.content_type.clone()
    };

    let mut badges = Vec::new();
    if item.is_pinned {
        badges.push("pinned".to_string());
    }
    if item.is_favorited {
        badges.push("favorite".to_string());
    }
    if item.kind == "snippet" {
        badges.push("snippet".to_string());
    }

    let title = item
        .display_title
        .clone()
        .or_else(|| item.preview_text.clone())
        .or_else(|| item.url.clone())
        .unwrap_or_else(|| "Untitled".to_string());

    let subtitle = match item.content_type.as_str() {
        "url" => item.url_domain.clone().or(item.url.clone()),
        "code" => Some(format!(
            "{} · {} lines",
            item.code_language.as_deref().unwrap_or("code"),
            item.line_count.unwrap_or(1)
        )),
        "image" => Some(format!(
            "{} KB",
            item.blob_size.unwrap_or(0) / 1024
        )),
        _ => item.preview_text.clone(),
    };

    let device = item.source_device_name.as_deref().unwrap_or("Unknown");
    let ago = format_relative(&item.created_at);
    let meta = match item.char_count {
        Some(c) => format!("{device} · {ago} · {c} chars"),
        None => format!("{device} · {ago}"),
    };

    PreviewCardDto {
        id: item.id.clone(),
        kind,
        title,
        subtitle,
        meta,
        thumbnail: item.thumbnail_path.clone(),
        badges,
        is_pinned: item.is_pinned,
        is_favorited: item.is_favorited,
    }
}

fn truncate_preview(text: &str) -> String {
    text.chars().take(200).collect()
}

fn extract_url_meta(url: &str) -> (Option<String>, Option<String>) {
    let domain = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .map(|s| s.to_string());
    (domain.clone(), domain)
}

fn detect_code(text: Option<&str>) -> (Option<String>, Option<i64>) {
    let Some(text) = text else {
        return (None, None);
    };
    let Some(first) = text.lines().next() else {
        return (None, None);
    };
    let lang = if first.contains("fn ") || first.contains("let ") {
        "Rust"
    } else if first.contains("def ") {
        "Python"
    } else if first.contains("function") || first.contains("const ") {
        "JavaScript"
    } else if first.contains("<?php") || text.contains("$") {
        "PHP"
    } else if first.starts_with("SELECT") || first.starts_with("INSERT") {
        "SQL"
    } else {
        "Code"
    };
    let lines = text.lines().count() as i64;
    (Some(lang.to_string()), Some(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (Database, tempdir::TempDirGuard) {
        let guard = tempdir::TempDirGuard::new();
        let db = Database::open(guard.path().join("test.db")).expect("open test db");
        (db, guard)
    }

    /// Minimal temp-dir helper (no extra dev-dependency).
    mod tempdir {
        use std::path::{Path, PathBuf};

        pub struct TempDirGuard(PathBuf);

        impl TempDirGuard {
            pub fn new() -> Self {
                let dir = std::env::temp_dir().join(format!("memora-test-{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(&dir).expect("create temp dir");
                Self(dir)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn insert_search_delete_keeps_fts_consistent() {
        let (db, _guard) = test_db();
        let device_id = db.ensure_device().expect("device");

        let item = db
            .insert_item(
                &device_id,
                "text",
                Some("hello transactional world".to_string()),
                None,
                None,
                None,
                None,
                "hash-1",
            )
            .expect("insert");

        let results = db
            .search(&SearchFiltersDto {
                query: "transactional".to_string(),
                device: None,
                content_type: None,
                tag: None,
                collection: None,
                is_pinned: None,
                is_favorite: None,
                is_snippet: None,
                date_today: None,
                in_collection: None,
            })
            .expect("search");
        assert_eq!(results.len(), 1, "inserted item should be searchable");

        db.delete_item(&item.id).expect("delete");
        let results = db
            .search(&SearchFiltersDto {
                query: "transactional".to_string(),
                device: None,
                content_type: None,
                tag: None,
                collection: None,
                is_pinned: None,
                is_favorite: None,
                is_snippet: None,
                date_today: None,
                in_collection: None,
            })
            .expect("search after delete");
        assert!(results.is_empty(), "deleted item must leave no ghost search hits");
    }

    #[test]
    fn pruning_stale_devices_reattributes_their_items() {
        let (db, _guard) = test_db();
        let current = db.ensure_device().expect("device");

        // Simulate a leftover device row from an old id rotation, with an
        // item still attributed to it.
        let stale_id = "stale-device-id".to_string();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO devices (id, name, platform, is_current, created_at) VALUES (?1, 'Old Row', 'windows', 0, '2026-01-01T00:00:00Z')",
                params![stale_id],
            )
            .expect("insert stale device");
        }
        let item = db
            .insert_item(&stale_id, "text", Some("orphan".to_string()), None, None, None, None, "h1")
            .expect("insert item");

        // Cloud no longer knows the stale row (it was merged server-side).
        let pruned = db
            .prune_local_devices_not_in(&[current.clone()], &current)
            .expect("prune");
        assert_eq!(pruned, 1);

        let devices = db.get_devices().expect("devices");
        assert!(devices.iter().all(|d| d.id != stale_id), "stale row removed");
        let item = db.get_item(&item.id).expect("item survives");
        assert_eq!(item.source_device_id.as_deref(), Some(current.as_str()));
    }

    #[test]
    fn duplicate_hash_within_window_dedupes() {
        let (db, _guard) = test_db();
        let device_id = db.ensure_device().expect("device");

        let first = db
            .insert_item(&device_id, "text", Some("dup".to_string()), None, None, None, None, "same-hash")
            .expect("insert 1");
        let second = db
            .insert_item(&device_id, "text", Some("dup".to_string()), None, None, None, None, "same-hash")
            .expect("insert 2");
        assert_eq!(first.id, second.id, "same hash within window should dedupe");
    }
}

fn format_relative(iso: &str) -> String {
    let Ok(parsed) = DateTime::parse_from_rfc3339(iso) else {
        return iso.to_string();
    };
    let diff = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
    let mins = diff.num_minutes();
    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else if mins < 1440 {
        format!("{}h ago", mins / 60)
    } else {
        parsed.with_timezone(&Local).format("%b %d").to_string()
    }
}
