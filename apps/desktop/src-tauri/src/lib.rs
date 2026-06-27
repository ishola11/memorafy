#![allow(unexpected_cfgs)]

pub mod clipboard;
pub mod commands;
pub mod db;
pub mod macos_popover;
pub mod macos_quick_paste;
pub mod search;
pub mod sync;
pub mod timeline;
#[cfg(not(target_os = "macos"))]
mod windows_tray;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, RunEvent, WebviewWindow, WindowEvent,
};
#[cfg(not(target_os = "macos"))]
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Ignore blur immediately after opening Quick Paste (shortcut / panel focus handoff).
static QUICK_PASTE_OPENED_AT: Mutex<Option<Instant>> = Mutex::new(None);
const QUICK_PASTE_BLUR_GRACE: Duration = Duration::from_millis(500);

pub struct AppState {
    pub db: Arc<db::Database>,
    pub suppress_clipboard: Arc<Mutex<u32>>,
    pub last_programmatic_hash: Arc<Mutex<Option<String>>>,
    pub last_capture: Arc<Mutex<Option<(String, Instant)>>>,
    pub clipboard_paused: Arc<AtomicBool>,
    pub sync_engine: Arc<sync::SyncEngine>,
    pub device_name: String,
    pub device_id: String,
}

#[cfg(target_os = "macos")]
fn app_builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Memora")
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_quick_paste(app, true);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_nspopover::init())
        .plugin(tauri_nspanel::init())
}

#[cfg(not(target_os = "macos"))]
fn app_builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Memora")
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_quick_paste(app, true);
                    }
                })
                .build(),
        )
}

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_config.rs"));
}

fn init_updater_plugin(app: &tauri::AppHandle) -> tauri::Result<()> {
    let Some(pubkey) = embedded::UPDATER_PUBKEY else {
        tracing::warn!("updater: no public key embedded at build time");
        return Ok(());
    };

    app.plugin(
        tauri_plugin_updater::Builder::new()
            .pubkey(pubkey)
            .build(),
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    app_builder()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            macos_popover::init_menubar_app_policy(app.handle());

            init_updater_plugin(app.handle())?;

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

            let clipboard_paused = database.get_clipboard_paused().unwrap_or(false);

            app.manage(AppState {
                db: database.clone(),
                suppress_clipboard: Arc::new(Mutex::new(0)),
                last_programmatic_hash: Arc::new(Mutex::new(None)),
                last_capture: Arc::new(Mutex::new(None)),
                clipboard_paused: Arc::new(AtomicBool::new(clipboard_paused)),
                sync_engine: sync_engine.clone(),
                device_name,
                device_id: device_id.clone(),
            });

            // System tray — macOS uses native NSPopover on left-click (full TrayPanel);
            // Windows keeps the custom sidebar panel on left-click.
            let show_i = MenuItem::with_id(app, "show", "Open Memora", true, None::<&str>)?;
            let quick_i =
                MenuItem::with_id(app, "quick", "Quick Paste", true, None::<&str>)?;
            let settings_i =
                MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quick_i, &settings_i, &quit_i])?;

            #[cfg(target_os = "macos")]
            {
                let tray = app
                    .tray_by_id("main")
                    .expect("tray id 'main' missing — add trayIcon to tauri.macos.conf.json");
                tray.set_menu(Some(menu))?;
                tray.set_show_menu_on_left_click(false)?;

                let handle = app.handle().clone();
                tray.on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => open_memora(app),
                    "quick" => toggle_quick_paste(app, true),
                    "settings" => {
                        let _ = commands::open_settings(app.clone());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
                tray.on_tray_icon_event(move |_, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        macos_popover::toggle_tray_nspopover(&handle);
                    }
                });

                macos_popover::setup_tray_nspopover(app.handle());
                macos_quick_paste::setup_quick_paste_panel(app.handle());
            }

            #[cfg(not(target_os = "macos"))]
            {
                let _tray = TrayIconBuilder::with_id("main")
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => open_memora(app),
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
                            rect,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            windows_tray::toggle_tray_window(&app, true, Some(rect));
                        }
                    })
                    .build(app)?;
            }

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
            commands::get_tab_timeline,
            commands::copy_item,
            commands::toggle_pin,
            commands::toggle_favorite,
            commands::delete_item,
            commands::rename_item,
            commands::create_snippet,
            commands::update_snippet,
            commands::save_item_as_snippet,
            commands::get_collections,
            commands::create_collection,
            commands::update_collection,
            commands::delete_collection,
            commands::add_item_to_collection,
            commands::remove_item_from_collection,
            commands::get_item_collections,
            commands::get_devices,
            commands::show_quick_paste,
            commands::hide_quick_paste,
            commands::get_item,
            commands::get_sync_state,
            commands::auth_login,
            commands::auth_logout,
            commands::get_app_settings,
            commands::set_history_retention,
            commands::preview_clear_history,
            commands::clear_history,
            commands::set_launch_at_login,
            commands::get_clipboard_paused,
            commands::toggle_clipboard_pause,
            commands::get_theme_preference,
            commands::set_theme_preference,
            commands::open_settings,
            commands::force_sync_now,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|app, event| {
            if matches!(event, RunEvent::Ready) {
                #[cfg(target_os = "macos")]
                {
                    macos_popover::retry_setup_tray_nspopover(app);
                    macos_quick_paste::retry_setup_quick_paste_panel(app);
                }
            }
            if let RunEvent::WindowEvent { label, event, .. } = event {
                if let WindowEvent::CloseRequested { api, .. } = &event {
                    if label == "settings" {
                        api.prevent_close();
                        if let Some(window) = app.get_webview_window(&label) {
                            let _ = window.hide();
                        }
                        #[cfg(target_os = "macos")]
                        macos_popover::restore_menubar_app_policy(app);
                    }
                }
                if let WindowEvent::Focused(focused) = event {
                    if !focused {
                        if label == "settings" {
                            #[cfg(target_os = "macos")]
                            macos_popover::restore_menubar_app_policy(app);
                        }
                        if label == "quick-paste" {
                            let skip_blur = QUICK_PASTE_OPENED_AT
                                .lock()
                                .as_ref()
                                .is_some_and(|t| t.elapsed() < QUICK_PASTE_BLUR_GRACE);
                            if skip_blur {
                                return;
                            }
                            hide_quick_paste(app);
                        }
                        if label == "tray" {
                            #[cfg(target_os = "macos")]
                            {
                                // NSPopover dismisses itself; no manual hide on blur.
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                if windows_tray::blur_grace_active() {
                                    return;
                                }
                                let _ = app.emit("tray-visibility", false);
                                if let Some(w) = app.get_webview_window("tray") {
                                    let _ = w.hide();
                                }
                            }
                        }
                    }
                }
            }
        });
}

/// Primary entry from tray menu — NSPopover history on macOS, sidebar panel on Windows.
fn open_memora(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    macos_popover::show_tray_nspopover(app);

    #[cfg(not(target_os = "macos"))]
    windows_tray::open_tray(app);
}

pub fn show_quick_paste(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("quick-paste") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        hide_quick_paste(app);
        return;
    }

    #[cfg(target_os = "macos")]
    macos_popover::hide_tray_nspopover(app);

    position_quick_paste(&window);
    macos_quick_paste::show_quick_paste_window(&window, app);
    *QUICK_PASTE_OPENED_AT.lock() = Some(Instant::now());
    let _ = app.emit("quick-paste-visibility", true);
}

pub fn hide_quick_paste(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("quick-paste") {
        if !window.is_visible().unwrap_or(false) {
            return;
        }
    }

    #[cfg(target_os = "macos")]
    macos_quick_paste::hide_quick_paste_panel(app);

    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window("quick-paste") {
        let _ = window.hide();
    }

    *QUICK_PASTE_OPENED_AT.lock() = None;
    let _ = app.emit("quick-paste-visibility", false);
}

fn toggle_quick_paste(app: &tauri::AppHandle, show: bool) {
    if show {
        show_quick_paste(app);
    } else {
        hide_quick_paste(app);
    }
}

/// Cover the active monitor so Quick Paste can dim the full screen (not a small floating card).
pub fn position_quick_paste(window: &WebviewWindow) {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|cursor| monitor_at_point(window, cursor.x as i32, cursor.y as i32))
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };

    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    let _ = window.set_position(PhysicalPosition::new(mon_pos.x, mon_pos.y));
    let _ = window.set_size(PhysicalSize::new(mon_size.width, mon_size.height));
}

fn monitor_at_point(window: &WebviewWindow, x: i32, y: i32) -> Option<Monitor> {
    window
        .available_monitors()
        .ok()?
        .into_iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            x >= pos.x
                && x < pos.x + size.width as i32
                && y >= pos.y
                && y < pos.y + size.height as i32
        })
}

