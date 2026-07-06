-- Memorafy local schema v1

CREATE TABLE IF NOT EXISTS devices (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  platform TEXT NOT NULL,
  last_seen_at TEXT,
  is_current INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS items (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL DEFAULT 'history',
  content_type TEXT NOT NULL,
  display_title TEXT,
  preview_text TEXT,
  char_count INTEGER,
  url TEXT,
  url_title TEXT,
  url_domain TEXT,
  code_language TEXT,
  line_count INTEGER,
  blob_path TEXT,
  blob_size INTEGER,
  thumbnail_path TEXT,
  content_hash TEXT NOT NULL,
  plain_text TEXT,
  trigger TEXT,
  source_device_id TEXT,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  is_favorited INTEGER NOT NULL DEFAULT 0,
  sync_status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  FOREIGN KEY (source_device_id) REFERENCES devices(id)
);

CREATE INDEX IF NOT EXISTS idx_items_created ON items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_items_pinned ON items(is_pinned) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_items_hash ON items(content_hash);

CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
  id UNINDEXED,
  display_title,
  preview_text,
  url_domain,
  tags,
  trigger,
  tokenize='porter unicode61'
);

CREATE TABLE IF NOT EXISTS tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS item_tags (
  item_id TEXT NOT NULL,
  tag_id TEXT NOT NULL,
  PRIMARY KEY (item_id, tag_id),
  FOREIGN KEY (item_id) REFERENCES items(id),
  FOREIGN KEY (tag_id) REFERENCES tags(id)
);

CREATE TABLE IF NOT EXISTS collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  color TEXT NOT NULL DEFAULT '#6366f1',
  icon TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS item_collections (
  item_id TEXT NOT NULL,
  collection_id TEXT NOT NULL,
  PRIMARY KEY (item_id, collection_id),
  FOREIGN KEY (item_id) REFERENCES items(id),
  FOREIGN KEY (collection_id) REFERENCES collections(id)
);

CREATE TABLE IF NOT EXISTS sync_queue (
  id TEXT PRIMARY KEY,
  op TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  payload TEXT NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_cursor (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  last_event_id TEXT,
  last_sync_at TEXT
);

INSERT OR IGNORE INTO sync_cursor (id) VALUES (1);
