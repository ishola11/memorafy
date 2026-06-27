use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use super::models::*;
use crate::timeline;

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
        self.conn.lock().unwrap().execute_batch(sql)
    }

    fn seed_defaults(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
        Ok(())
    }

    pub fn blobs_dir(&self) -> &Path {
        &self.blobs_dir
    }

    pub fn ensure_device(&self) -> Result<String, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        if let Ok(id) = conn.query_row(
            "SELECT id FROM devices WHERE is_current = 1 LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        ) {
            return Ok(id);
        }

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

        conn.execute("UPDATE devices SET is_current = 0", [])?;
        conn.execute(
            "INSERT INTO devices (id, name, platform, last_seen_at, is_current, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            params![id, name, platform, now, now],
        )?;
        Ok(id)
    }

    pub fn get_device_name(&self, device_id: &str) -> Result<String, rusqlite::Error> {
        self.conn.lock().unwrap().query_row(
            "SELECT name FROM devices WHERE id = ?1",
            params![device_id],
            |r| r.get(0),
        )
    }

    pub fn touch_device(&self, device_id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        self.conn.lock().unwrap().execute(
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
        // Dedupe: same hash within 2 seconds on this device
        let conn = self.conn.lock().unwrap();
        let recent: Option<String> = conn
            .query_row(
                "SELECT id FROM items WHERE content_hash = ?1 AND source_device_id = ?2
                 AND deleted_at IS NULL AND datetime(created_at) > datetime('now', '-2 seconds')
                 LIMIT 1",
                params![content_hash, device_id],
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

        conn.execute(
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
        conn.execute(
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

        self.enqueue_sync(&conn, "create", "item", &id)?;

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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();

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

    pub fn toggle_pin(&self, id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET is_pinned = CASE is_pinned WHEN 1 THEN 0 ELSE 1 END, updated_at = ?1, sync_status = 'pending' WHERE id = ?2",
            params![now, id],
        )?;
        self.enqueue_sync(&conn, "update", "item", id)?;
        Ok(())
    }

    pub fn toggle_favorite(&self, id: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET is_favorited = CASE is_favorited WHEN 1 THEN 0 ELSE 1 END, updated_at = ?1, sync_status = 'pending' WHERE id = ?2",
            params![now, id],
        )?;
        self.enqueue_sync(&conn, "update", "item", id)?;
        Ok(())
    }

    pub fn delete_item(&self, id: &str) -> Result<(), rusqlite::Error> {
        self.soft_delete_item(id, true)
    }

    fn soft_delete_item(&self, id: &str, remove_blob: bool) -> Result<(), rusqlite::Error> {
        let blob_path: Option<String> = if remove_blob {
            self.conn.lock().unwrap().query_row(
                "SELECT blob_path FROM items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            ).optional()?.flatten()
        } else {
            None
        };

        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET deleted_at = ?1, updated_at = ?1, sync_status = 'pending' WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
        conn.execute("DELETE FROM items_fts WHERE id = ?1", params![id])?;
        self.enqueue_sync(&conn, "delete", "item", id)?;
        drop(conn);

        if let Some(path) = blob_path {
            std::fs::remove_file(path).ok();
        }
        Ok(())
    }

    pub fn apply_remote_deletion(&self, id: &str, deleted_at: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET deleted_at = ?1, sync_status = 'synced' WHERE id = ?2 AND deleted_at IS NULL",
            params![deleted_at, id],
        )?;
        conn.execute("DELETE FROM items_fts WHERE id = ?1", params![id])?;
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
        })
    }

    /// Soft-delete expired history clips. Keeps pinned, favorited, snippets, and collection items.
    pub fn purge_expired_history(&self) -> Result<u32, rusqlite::Error> {
        let days = self.get_history_retention_days()?;
        if days <= 0 {
            return Ok(0);
        }

        let cutoff = format!("-{days} days");
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id FROM items i
             WHERE i.deleted_at IS NULL
               AND i.kind = 'history'
               AND i.is_pinned = 0
               AND i.is_favorited = 0
               AND i.id NOT IN (SELECT item_id FROM item_collections)
               AND datetime(i.created_at) < datetime('now', ?1)
             LIMIT 500",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![cutoff], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        drop(conn);

        let mut purged = 0u32;
        for id in ids {
            if self.soft_delete_item(&id, true).is_ok() {
                purged += 1;
            }
        }
        Ok(purged)
    }

    pub fn rename_item(&self, id: &str, title: &str) -> Result<(), rusqlite::Error> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET display_title = ?1, updated_at = ?2, sync_status = 'pending' WHERE id = ?3",
            params![title, now, id],
        )?;
        conn.execute(
            "UPDATE items_fts SET display_title = ?1 WHERE id = ?2",
            params![title, id],
        )?;
        self.enqueue_sync(&conn, "update", "item", id)?;
        Ok(())
    }

    pub fn get_collections(&self) -> Result<Vec<CollectionDto>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.name, c.color, c.icon,
                    (SELECT COUNT(*) FROM item_collections ic WHERE ic.collection_id = c.id) as cnt
             FROM collections c ORDER BY c.sort_order, c.name",
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

    pub fn get_devices(&self) -> Result<Vec<DeviceDto>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
        self.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status = 'pending'",
            [],
            |r| r.get(0),
        )
    }

    pub fn mark_synced(&self, entity_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.conn.lock().unwrap().query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        ).optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), rusqlite::Error> {
        self.conn.lock().unwrap().execute(
            "DELETE FROM settings WHERE key = ?1",
            params![key],
        )?;
        Ok(())
    }

    pub fn list_pending_sync_items(&self) -> Result<Vec<ItemRecord>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
        let count: i64 = self.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM items WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn upsert_remote_item(&self, item: &crate::sync::client::CloudItem) -> Result<(), rusqlite::Error> {
        if let Some(ref device_id) = item.source_device_id {
            self.ensure_device_exists(device_id)?;
        }

        let conn = self.conn.lock().unwrap();
        conn.execute(
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
                sync_status = 'synced'",
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
                item.blob_path,
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

        conn.execute("DELETE FROM items_fts WHERE id = ?1", params![item.id])?;
        conn.execute(
            "INSERT INTO items_fts (id, display_title, preview_text, url_domain, tags, trigger)
             VALUES (?1, ?2, ?3, ?4, '', ?5)",
            params![
                item.id,
                item.display_title,
                item.preview_text,
                item.url_domain,
                item.trigger,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_remote_device(&self, device: &crate::sync::client::CloudDevice) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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

    fn ensure_device_exists(&self, device_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
