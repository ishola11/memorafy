use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::clipboard::{write_clipboard, write_clipboard_image, write_clipboard_rich};
use crate::db::{item_to_preview, PreviewCardDto, SearchFiltersDto, TabFiltersDto, TimelineSectionDto};
use crate::AppState;

#[tauri::command]
pub fn search_items(
    state: State<'_, AppState>,
    filters: SearchFiltersDto,
) -> Result<Vec<PreviewCardDto>, String> {
    state
        .db
        .search(&filters)
        .map(|items| items.iter().map(item_to_preview).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_timeline(state: State<'_, AppState>) -> Result<Vec<TimelineSectionDto>, String> {
    state.db.get_timeline().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tab_timeline(
    state: State<'_, AppState>,
    filters: TabFiltersDto,
) -> Result<Vec<TimelineSectionDto>, String> {
    state
        .db
        .get_tab_timeline(&filters.tab, filters.collection_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_clipboard_paused(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.clipboard_paused.load(std::sync::atomic::Ordering::Relaxed))
}

#[tauri::command]
pub fn toggle_clipboard_pause(state: State<'_, AppState>) -> Result<bool, String> {
    let paused = !state.clipboard_paused.load(std::sync::atomic::Ordering::Relaxed);
    state
        .clipboard_paused
        .store(paused, std::sync::atomic::Ordering::Relaxed);
    state.db.set_clipboard_paused(paused).map_err(|e| e.to_string())?;
    Ok(paused)
}

/// `plain_text = true` guarantees the literal characters with no formatting
/// — useful when a rich hyperlink/object would be unwanted (pasting into a
/// filename, terminal, or code editor). `plain_text = false` enriches the
/// paste where formatting carries real information (currently: a clickable
/// link for copied URLs); for content types with nothing to enrich, both
/// options produce the same plain-text result.
#[tauri::command]
pub fn copy_item(state: State<'_, AppState>, id: String, plain_text: bool) -> Result<(), String> {
    let item = state.db.get_item(&id).map_err(|e| e.to_string())?;

    if item.content_type == "image" {
        if plain_text {
            if let Some(label) = item.preview_text.as_deref() {
                return write_clipboard(&state, label).map_err(|e| e.to_string());
            }
            return Err("This image has no plain-text representation.".into());
        }
        let blob_path = item
            .blob_path
            .as_deref()
            .ok_or("Image file is missing on this device.")?;
        return write_clipboard_image(
            &state,
            std::path::Path::new(blob_path),
            item.preview_text.as_deref(),
        )
        .map_err(|e| e.to_string());
    }

    let Some(text) = item.plain_text.clone() else {
        return Err("Nothing to copy for this item.".into());
    };

    if !plain_text {
        if let Some(html) = rich_html_for_item(&item) {
            return write_clipboard_rich(&state, &text, &html).map_err(|e| e.to_string());
        }
    }

    write_clipboard(&state, &text).map_err(|e| e.to_string())
}

/// Builds a rich-paste representation for content types where formatting is
/// meaningful. Returns `None` when plain text is already the richest useful
/// representation (text, code, snippets — Memora doesn't capture original
/// HTML/RTF for these, so there is nothing genuine to enrich).
fn rich_html_for_item(item: &crate::db::ItemRecord) -> Option<String> {
    if item.content_type != "url" {
        return None;
    }
    let url = item.url.as_deref()?;
    let label = item
        .url_title
        .as_deref()
        .or(item.url_domain.as_deref())
        .unwrap_or(url);
    Some(format!(
        r#"<a href="{}">{}</a>"#,
        html_escape_attr(url),
        html_escape_text(label)
    ))
}

/// The href value lands in other applications' clipboard/paste handling, so
/// it must not break out of the attribute even though it's our own data.
fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod copy_item_tests {
    use super::rich_html_for_item;
    use crate::db::ItemRecord;

    fn base_item() -> ItemRecord {
        ItemRecord {
            id: "1".into(),
            kind: "history".into(),
            content_type: "url".into(),
            display_title: None,
            preview_text: None,
            char_count: None,
            url: Some("https://example.com/a?b=1&c=2".into()),
            url_title: None,
            url_domain: Some("example.com".into()),
            code_language: None,
            line_count: None,
            blob_path: None,
            blob_size: None,
            thumbnail_path: None,
            content_hash: "hash".into(),
            plain_text: Some("https://example.com/a?b=1&c=2".into()),
            trigger: None,
            source_device_id: None,
            source_device_name: None,
            is_pinned: false,
            is_favorited: false,
            sync_status: "synced".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deleted_at: None,
        }
    }

    #[test]
    fn url_items_get_an_escaped_html_link() {
        let item = base_item();
        let html = rich_html_for_item(&item).expect("url should produce html");
        assert!(html.contains(r#"href="https://example.com/a?b=1&amp;c=2""#));
        assert!(html.contains(">example.com<"));
    }

    #[test]
    fn non_url_items_have_no_rich_representation() {
        let mut item = base_item();
        item.content_type = "text".into();
        assert!(rich_html_for_item(&item).is_none());
    }
}

#[tauri::command]
pub fn toggle_pin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.toggle_pin(&id).map_err(|e| e.to_string())?;
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.toggle_favorite(&id).map_err(|e| e.to_string())?;
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn delete_item(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_item(&id).map_err(|e| e.to_string())?;
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn rename_item(state: State<'_, AppState>, id: String, title: String) -> Result<(), String> {
    state.db.rename_item(&id, &title).map_err(|e| e.to_string())?;
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn create_snippet(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::db::CreateSnippetDto,
) -> Result<crate::db::ItemRecord, String> {
    let item = state
        .db
        .create_snippet(
            &state.device_id(),
            &input.title,
            &input.text,
            input.trigger.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    let _ = app.emit("items-updated", ());
    state.sync_engine.request_sync();
    Ok(item)
}

#[tauri::command]
pub fn update_snippet(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::db::UpdateSnippetDto,
) -> Result<crate::db::ItemRecord, String> {
    let item = state
        .db
        .update_snippet(
            &input.id,
            &input.title,
            &input.text,
            input.trigger.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    let _ = app.emit("items-updated", ());
    state.sync_engine.request_sync();
    Ok(item)
}

#[tauri::command]
pub fn save_item_as_snippet(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::db::ItemRecord, String> {
    let item = state.db.save_item_as_snippet(&id).map_err(|e| e.to_string())?;
    let _ = app.emit("items-updated", ());
    state.sync_engine.request_sync();
    Ok(item)
}

#[tauri::command]
pub fn get_collections(state: State<'_, AppState>) -> Result<Vec<crate::db::CollectionDto>, String> {
    state.db.get_collections().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::db::CreateCollectionDto,
) -> Result<crate::db::CollectionDto, String> {
    let collection = state
        .db
        .create_collection(&input.name, &input.color)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("collections-updated", ());
    state.sync_engine.request_sync();
    Ok(collection)
}

#[tauri::command]
pub fn update_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::db::UpdateCollectionDto,
) -> Result<crate::db::CollectionDto, String> {
    let collection = state
        .db
        .update_collection(
            &input.id,
            input.name.as_deref(),
            input.color.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    let _ = app.emit("collections-updated", ());
    state.sync_engine.request_sync();
    Ok(collection)
}

#[tauri::command]
pub fn delete_collection(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_collection(&id).map_err(|e| e.to_string())?;
    let _ = app.emit("collections-updated", ());
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn add_item_to_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: String,
    collection_id: String,
) -> Result<(), String> {
    state
        .db
        .add_item_to_collection(&item_id, &collection_id)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("collections-updated", ());
    let _ = app.emit("items-updated", ());
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn remove_item_from_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: String,
    collection_id: String,
) -> Result<(), String> {
    state
        .db
        .remove_item_from_collection(&item_id, &collection_id)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("collections-updated", ());
    let _ = app.emit("items-updated", ());
    state.sync_engine.request_sync();
    Ok(())
}

#[tauri::command]
pub fn get_item_collections(state: State<'_, AppState>, item_id: String) -> Result<Vec<String>, String> {
    state
        .db
        .get_item_collection_ids(&item_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_theme_preference(state: State<'_, AppState>) -> Result<String, String> {
    state.db.get_theme_preference().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_theme_preference(
    app: AppHandle,
    state: State<'_, AppState>,
    preference: String,
) -> Result<String, String> {
    state
        .db
        .set_theme_preference(&preference)
        .map_err(|e| e.to_string())?;
    let pref = state.db.get_theme_preference().map_err(|e| e.to_string())?;
    app.emit("theme-changed", &pref)
        .map_err(|e| e.to_string())?;
    Ok(pref)
}

#[tauri::command]
pub async fn get_devices(state: State<'_, AppState>) -> Result<Vec<crate::db::DeviceDto>, String> {
    // Refresh from the cloud when possible so the Devices page shows the
    // deduplicated, current list instead of a stale local snapshot; falls
    // back silently to local data offline.
    state.sync_engine.refresh_devices().await;
    state.db.get_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_item(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<crate::db::ItemRecord>, String> {
    match state.db.get_item(&id) {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn show_quick_paste(app: AppHandle) -> Result<(), String> {
    crate::show_quick_paste(&app);
    Ok(())
}

#[tauri::command]
pub fn hide_quick_paste(app: AppHandle) -> Result<(), String> {
    crate::hide_quick_paste(&app);
    Ok(())
}

#[tauri::command]
pub fn get_sync_state(state: State<'_, AppState>) -> Result<crate::sync::SyncStateDto, String> {
    state.sync_engine.get_state()
}

#[tauri::command]
pub async fn auth_login(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<crate::sync::SyncStateDto, String> {
    state.sync_engine.login(&email, &password).await
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<crate::sync::SyncStateDto, String> {
    state.sync_engine.logout()
}

#[tauri::command]
pub async fn auth_signup(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<crate::sync::SignUpResultDto, String> {
    if password.chars().count() < 8 {
        return Err("Password must be at least 8 characters.".into());
    }
    state.sync_engine.sign_up(email.trim(), &password).await
}

#[tauri::command]
pub async fn auth_resend_confirmation(
    state: State<'_, AppState>,
    email: String,
) -> Result<(), String> {
    state.sync_engine.resend_confirmation(email.trim()).await
}

#[tauri::command]
pub async fn auth_request_password_reset(
    state: State<'_, AppState>,
    email: String,
) -> Result<(), String> {
    state
        .sync_engine
        .request_password_reset(email.trim())
        .await
}

#[tauri::command]
pub async fn auth_change_password(
    state: State<'_, AppState>,
    new_password: String,
) -> Result<(), String> {
    if new_password.chars().count() < 8 {
        return Err("Password must be at least 8 characters.".into());
    }
    state.sync_engine.change_password(&new_password).await
}

#[tauri::command]
pub async fn unlock_sync_encryption(
    state: State<'_, AppState>,
    password: String,
) -> Result<crate::sync::SyncStateDto, String> {
    state.sync_engine.unlock_encryption(&password).await
}

#[tauri::command]
pub async fn reset_sync_encryption(
    state: State<'_, AppState>,
    password: String,
) -> Result<crate::sync::SyncStateDto, String> {
    state.sync_engine.reset_encryption(&password).await
}

/// Erase every trace of local data and restart fresh: clipboard history,
/// blobs, settings, cached session, and encryption key. Cloud data is NOT
/// touched — that's History → Clear (Everywhere). The actual file deletion
/// happens on next launch via a sentinel, because this process still holds
/// the database open.
#[tauri::command]
pub fn erase_all_data(app: AppHandle) -> Result<(), String> {
    if let Err(e) = crate::keychain::clear() {
        tracing::warn!("erase: keychain session clear: {e}");
    }
    if let Err(e) = crate::keychain::clear_dek() {
        tracing::warn!("erase: keychain key clear: {e}");
    }

    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    std::fs::write(app_data.join("reset_pending"), b"1")
        .map_err(|e| format!("could not schedule the reset: {e}"))?;

    tracing::warn!("local data erase scheduled — restarting");
    app.restart();
}

const SETTING_ONBOARDING_COMPLETED: &str = "onboarding_completed";

#[tauri::command]
pub fn get_onboarding_completed(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state
        .db
        .get_setting(SETTING_ONBOARDING_COMPLETED)
        .map_err(|e| e.to_string())?
        .is_some())
}

#[tauri::command]
pub fn set_onboarding_completed(state: State<'_, AppState>) -> Result<(), String> {
    state
        .db
        .set_setting(SETTING_ONBOARDING_COMPLETED, "1")
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn force_sync_now(
    state: State<'_, AppState>,
) -> Result<crate::sync::SyncActionResultDto, String> {
    state.sync_engine.force_sync_now().await
}

#[tauri::command]
pub async fn repair_sync(
    state: State<'_, AppState>,
) -> Result<crate::sync::SyncRepairResultDto, String> {
    state.sync_engine.repair_sync().await
}

#[tauri::command]
pub fn preview_clear_history(
    state: State<'_, AppState>,
) -> Result<crate::db::ClearHistoryPreviewDto, String> {
    state
        .db
        .preview_clear_history()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: String,
    mode: String,
) -> Result<crate::db::ClearHistoryResultDto, String> {
    let scope = crate::db::ClearHistoryScope::parse(&scope)
        .ok_or_else(|| format!("invalid scope: {scope}"))?;
    let mode = crate::db::ClearHistoryMode::parse(&mode)
        .ok_or_else(|| format!("invalid mode: {mode}"))?;

    if scope == crate::db::ClearHistoryScope::Everywhere
        && !state.sync_engine.get_state()?.logged_in
    {
        return Err("Sign in to delete history from your cloud account.".into());
    }

    let result = state
        .db
        .clear_history(scope, mode)
        .map_err(|e| e.to_string())?;

    if scope == crate::db::ClearHistoryScope::Everywhere && result.cleared > 0 {
        let _ = state.sync_engine.force_sync_now().await;
    }

    let _ = app.emit("items-updated", ());
    Ok(result)
}

#[tauri::command]
pub fn get_app_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::db::AppSettingsDto, String> {
    let mut settings = state.db.get_app_settings().map_err(|e| e.to_string())?;
    settings.launch_at_login = app
        .autolaunch()
        .is_enabled()
        .map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| e.to_string())?;
    } else {
        autolaunch.disable().map_err(|e| e.to_string())?;
    }
    autolaunch.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_history_retention(
    app: AppHandle,
    state: State<'_, AppState>,
    days: i64,
) -> Result<crate::db::AppSettingsDto, String> {
    state
        .db
        .set_history_retention_days(days)
        .map_err(|e| e.to_string())?;
    let purged = state.sync_engine.run_retention_now()?;
    if purged > 0 {
        tracing::info!("retention setting change removed {purged} items");
    }
    get_app_settings(app, state)
}

/// Open a path or URL with the platform's default handler.
fn open_external(target: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(target).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(target).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(target).spawn();

    result
        .map(|_| ())
        .map_err(|e| format!("Could not open {target}: {e}"))
}

#[tauri::command]
pub fn open_logs_dir() -> Result<(), String> {
    let dir = crate::logging::log_dir().ok_or("Log directory is unavailable")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create log directory: {e}"))?;
    open_external(&dir.to_string_lossy())
}

#[tauri::command]
pub fn get_diagnostics(
    state: State<'_, AppState>,
    include_logs: bool,
) -> Result<crate::feedback::DiagnosticsDto, String> {
    crate::feedback::collect_diagnostics(&state, include_logs)
}

#[tauri::command]
pub fn submit_feedback(
    report: crate::feedback::FeedbackReport,
) -> Result<crate::feedback::FeedbackOutcome, String> {
    let provider = crate::feedback::default_provider();
    let outcome = provider.submit(&report)?;
    tracing::info!(provider = provider.name(), kind = %report.kind, "feedback submitted");
    match &outcome {
        crate::feedback::FeedbackOutcome::OpenUrl { url } => open_external(url)?,
    }
    Ok(outcome)
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    crate::macos_popover::activate_settings_policy(&app);

    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        app.emit("settings-visibility", true)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
