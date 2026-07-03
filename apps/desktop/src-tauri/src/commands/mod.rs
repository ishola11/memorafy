use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::clipboard::write_clipboard;
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

#[tauri::command]
pub fn copy_item(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let item = state.db.get_item(&id).map_err(|e| e.to_string())?;
    if let Some(text) = item.plain_text {
        write_clipboard(&state, &text).map_err(|e| e.to_string())?;
    }
    Ok(())
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
pub fn get_devices(state: State<'_, AppState>) -> Result<Vec<crate::db::DeviceDto>, String> {
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
