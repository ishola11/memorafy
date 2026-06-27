use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRecord {
    pub id: String,
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
    pub thumbnail_path: Option<String>,
    pub content_hash: String,
    pub plain_text: Option<String>,
    pub trigger: Option<String>,
    pub source_device_id: Option<String>,
    pub source_device_name: Option<String>,
    pub is_pinned: bool,
    pub is_favorited: bool,
    pub sync_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCardDto {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub meta: String,
    pub thumbnail: Option<String>,
    pub badges: Vec<String>,
    pub is_pinned: bool,
    pub is_favorited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSectionDto {
    pub bucket: String,
    pub label: String,
    pub items: Vec<PreviewCardDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFiltersDto {
    pub query: String,
    pub device: Option<String>,
    pub content_type: Option<String>,
    pub tag: Option<String>,
    pub collection: Option<String>,
    pub is_pinned: Option<bool>,
    pub is_favorite: Option<bool>,
    pub is_snippet: Option<bool>,
    pub date_today: Option<bool>,
    pub in_collection: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub item_count: i64,
}

pub const SETTING_HISTORY_RETENTION: &str = "history_retention_days";
pub const SETTING_CLIPBOARD_PAUSED: &str = "clipboard_paused";
pub const SETTING_THEME_PREFERENCE: &str = "theme_preference";
pub const DEFAULT_HISTORY_RETENTION_DAYS: i64 = 30;
pub const DEFAULT_THEME_PREFERENCE: &str = "system";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub history_retention_days: i64,
    pub clipboard_paused: bool,
    pub theme_preference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionDto {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionDto {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabFiltersDto {
    pub tab: String,
    pub collection_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDto {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub last_seen_at: Option<String>,
    pub is_current: bool,
    pub is_online: bool,
}
