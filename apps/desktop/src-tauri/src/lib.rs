pub mod clipboard;
pub mod commands;
pub mod db;
pub mod search;
pub mod sync;
pub mod timeline;

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, RunEvent, WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub struct AppState {
    pub db: Arc<db::Database>,
    pub suppress_clipboard: Arc<Mutex<u32>>,
    pub sync_engine: Arc<sync::SyncEngine>,
    pub device_name: String,
    pub device_id: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_quick_paste(app, true);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let app_data = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&app_data).ok();

            let database = Arc::new(db::Database::open(app_data.join("memora.db"))?);
            let device_id = database.ensure_device()?;
            let device_name = database.get_device_name(&device_id)?;
            database.set_setting("local_device_id", &device_id)?;
            database.set_setting("local_device_name", &device_name)?;

            let sync_engine = Arc::new(sync::SyncEngine::new(
                database.clone(),
                app.handle().clone(),
            ));

            app.manage(AppState {
                db: database.clone(),
                suppress_clipboard: Arc::new(Mutex::new(0)),
                sync_engine: sync_engine.clone(),
                device_name,
                device_id: device_id.clone(),
            });

            // System tray
            let show_i = MenuItem::with_id(app, "show", "Open Memora", true, None::<&str>)?;
            let quick_i =
                MenuItem::with_id(app, "quick", "Quick Paste", true, None::<&str>)?;
            let settings_i =
                MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quick_i, &settings_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_tray_window(app, true),
                    "quick" => toggle_quick_paste(app, true),
                    "settings" => {
                        let _ = commands::open_settings(app.clone());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_tray_window(&app, true);
                    }
                })
                .build(app)?;

            // Global shortcut: Ctrl+Shift+V (Windows) / Cmd+Shift+V (macOS)
            #[cfg(target_os = "macos")]
            let modifiers = Modifiers::SUPER | Modifiers::SHIFT;
            #[cfg(not(target_os = "macos"))]
            let modifiers = Modifiers::CONTROL | Modifiers::SHIFT;

            let shortcut = Shortcut::new(Some(modifiers), Code::KeyV);
            app.global_shortcut().register(shortcut)?;

            // Start clipboard watcher
            let handle = app.handle().clone();
            clipboard::start_watcher(handle);

            // Start sync engine
            sync_engine.clone().run_retention_if_due();
            sync_engine.start();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_items,
            commands::get_timeline,
            commands::copy_item,
            commands::toggle_pin,
            commands::toggle_favorite,
            commands::delete_item,
            commands::rename_item,
            commands::get_collections,
            commands::get_devices,
            commands::show_quick_paste,
            commands::hide_quick_paste,
            commands::get_item,
            commands::get_sync_state,
            commands::auth_login,
            commands::auth_logout,
            commands::get_app_settings,
            commands::set_history_retention,
            commands::open_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|app, event| {
            if let RunEvent::ExitRequested { api, .. } = &event {
                api.prevent_exit();
            }
            if let RunEvent::WindowEvent { label, event, .. } = event {
                if let WindowEvent::Focused(focused) = event {
                    if !focused {
                        if label == "quick-paste" {
                            let _ = app.emit("quick-paste-visibility", false);
                            if let Some(w) = app.get_webview_window("quick-paste") {
                                let _ = w.hide();
                            }
                        }
                        if label == "tray" {
                            let _ = app.emit("tray-visibility", false);
                            if let Some(w) = app.get_webview_window("tray") {
                                let _ = w.hide();
                            }
                        }
                    }
                }
            }
        });
}

fn toggle_quick_paste(app: &tauri::AppHandle, show: bool) {
    if let Some(window) = app.get_webview_window("quick-paste") {
        if show {
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("quick-paste-visibility", true);
        } else {
            let _ = window.hide();
            let _ = app.emit("quick-paste-visibility", false);
        }
    }
}

fn toggle_tray_window(app: &tauri::AppHandle, show: bool) {
    if let Some(window) = app.get_webview_window("tray") {
        if show {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("tray-visibility", true);
        } else {
            let _ = window.hide();
            let _ = app.emit("tray-visibility", false);
        }
    }
}
