use tauri::{AppHandle, Emitter, Manager, State};

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
pub fn copy_item(
    state: State<'_, AppState>,
    id: String,
    plain_text: bool,
) -> Result<(), String> {
    let item = state.db.get_item(&id).map_err(|e| e.to_string())?;
    let text = if plain_text {
        item.plain_text.clone()
    } else {
        item.plain_text.clone()
    };
    if let Some(t) = text {
        write_clipboard(&state, &t).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_pin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.toggle_pin(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_favorite(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.toggle_favorite(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_item(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_item(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_item(state: State<'_, AppState>, id: String, title: String) -> Result<(), String> {
    state.db.rename_item(&id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_collections(state: State<'_, AppState>) -> Result<Vec<crate::db::CollectionDto>, String> {
    state.db.get_collections().map_err(|e| e.to_string())
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
    if let Some(window) = app.get_webview_window("quick-paste") {
        window.center().map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        app.emit("quick-paste-visibility", true)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn hide_quick_paste(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick-paste") {
        window.hide().map_err(|e| e.to_string())?;
        app.emit("quick-paste-visibility", false)
            .map_err(|e| e.to_string())?;
    }
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
pub fn get_app_settings(state: State<'_, AppState>) -> Result<crate::db::AppSettingsDto, String> {
    state.db.get_app_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_history_retention(
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
    state.db.get_app_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        app.emit("settings-visibility", true)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
