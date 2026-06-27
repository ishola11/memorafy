#![allow(unexpected_cfgs)]

pub mod clipboard;
pub mod commands;
pub mod db;
pub mod macos_popover;
pub mod search;
pub mod sync;
pub mod timeline;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Monitor, PhysicalPosition, Position, RunEvent, Size, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Grace period after opening tray popover — ignore blur so the menubar click doesn't instantly hide it.
static TRAY_OPENED_AT: Mutex<Option<Instant>> = Mutex::new(None);
const TRAY_BLUR_GRACE: Duration = Duration::from_millis(450);

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

            // System tray — macOS uses native menu on left-click (Parallel/Paste style);
            // Windows keeps the custom sidebar panel on left-click.
            let show_i = MenuItem::with_id(app, "show", "Open Memora", true, None::<&str>)?;
            let quick_i =
                MenuItem::with_id(app, "quick", "Quick Paste", true, None::<&str>)?;
            let settings_i =
                MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quick_i, &settings_i, &quit_i])?;

            let mut tray_builder = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu);

            #[cfg(target_os = "macos")]
            {
                tray_builder = tray_builder
                    .icon_as_template(true)
                    .show_menu_on_left_click(true);
            }

            #[cfg(not(target_os = "macos"))]
            {
                tray_builder = tray_builder.show_menu_on_left_click(false);
            }

            let _tray = tray_builder
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
                    #[cfg(not(target_os = "macos"))]
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_tray_window(&app, true, Some(rect));
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
            commands::get_tab_timeline,
            commands::copy_item,
            commands::toggle_pin,
            commands::toggle_favorite,
            commands::delete_item,
            commands::rename_item,
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
            commands::get_clipboard_paused,
            commands::toggle_clipboard_pause,
            commands::get_theme_preference,
            commands::set_theme_preference,
            commands::open_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|app, event| {
            if matches!(event, RunEvent::Ready) {
                #[cfg(target_os = "macos")]
                macos_popover::init_menubar_app_policy(app);
            }
            if let RunEvent::ExitRequested { api, .. } = &event {
                api.prevent_exit();
            }
            if let RunEvent::WindowEvent { label, event, .. } = event {
                if let WindowEvent::CloseRequested { .. } = event {
                    if label == "settings" {
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
                            let _ = app.emit("quick-paste-visibility", false);
                            if let Some(w) = app.get_webview_window("quick-paste") {
                                let _ = w.hide();
                            }
                        }
                        if label == "tray" {
                            let skip_blur = TRAY_OPENED_AT
                                .lock()
                                .as_ref()
                                .is_some_and(|t| t.elapsed() < TRAY_BLUR_GRACE);
                            if skip_blur {
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
        });
}

/// Primary entry from tray menu — Quick Paste on macOS, sidebar panel on Windows.
fn open_memora(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    toggle_quick_paste(app, true);

    #[cfg(not(target_os = "macos"))]
    toggle_tray_window(app, true, None);
}

fn toggle_quick_paste(app: &tauri::AppHandle, show: bool) {
    if let Some(window) = app.get_webview_window("quick-paste") {
        if show {
            position_quick_paste(&window);
            macos_popover::show_popover_window(app, &window);
            let _ = window.set_focus();
            let _ = app.emit("quick-paste-visibility", true);
        } else {
            let _ = window.hide();
            let _ = app.emit("quick-paste-visibility", false);
        }
    }
}

fn toggle_tray_window(
    app: &tauri::AppHandle,
    show: bool,
    tray_rect: Option<tauri::Rect>,
) {
    if let Some(window) = app.get_webview_window("tray") {
        let is_visible = window.is_visible().unwrap_or(false);
        if show && is_visible {
            let _ = window.hide();
            let _ = app.emit("tray-visibility", false);
            return;
        }
        if show {
            if let Some(rect) = tray_rect {
                position_tray_panel(&window, rect);
            } else {
                position_tray_panel_fallback(&window);
            }
            macos_popover::show_popover_window(app, &window);
            *TRAY_OPENED_AT.lock() = Some(Instant::now());
            let _ = app.emit("tray-visibility", true);
        } else {
            let _ = window.hide();
            let _ = app.emit("tray-visibility", false);
        }
    }
}

/// Anchor the tray panel directly below the menubar / taskbar tray icon.
fn position_tray_panel(window: &WebviewWindow, tray_rect: tauri::Rect) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(400, 580));

    let (tray_x, tray_y, tray_w, tray_h) = rect_physical_bounds(&tray_rect, scale);
    let tray_center_x = tray_x + tray_w / 2;

    let monitor = monitor_at_point(window, tray_center_x, tray_y + tray_h / 2)
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };

    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let gap = 4i32;

    let mut x = tray_center_x - (size.width as i32 / 2);
    let min_x = mon_pos.x + 8;
    let max_x = mon_pos.x + mon_size.width as i32 - size.width as i32 - 8;
    x = x.clamp(min_x, max_x);

    #[cfg(target_os = "macos")]
    let y = tray_y + tray_h + gap;

    #[cfg(not(target_os = "macos"))]
    let y = {
        let below_tray = tray_y + tray_h + gap;
        let above_taskbar = mon_pos.y + mon_size.height as i32 - size.height as i32 - 8;
        if tray_y > mon_pos.y + mon_size.height as i32 / 2 {
            above_taskbar
        } else {
            below_tray
        }
    };

    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Center quick-paste on the monitor under the cursor (active screen), not the primary desktop.
pub fn position_quick_paste(window: &WebviewWindow) {
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(680, 480));

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
    let x = mon_pos.x + (mon_size.width as i32 - size.width as i32) / 2;
    let y = mon_pos.y + (mon_size.height as i32 - size.height as i32) / 4;

    let _ = window.set_position(PhysicalPosition::new(x, y));
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

fn rect_physical_bounds(rect: &tauri::Rect, scale: f64) -> (i32, i32, i32, i32) {
    let (x, y) = match rect.position {
        Position::Physical(p) => (p.x, p.y),
        Position::Logical(p) => ((p.x * scale).round() as i32, (p.y * scale).round() as i32),
    };
    let (w, h) = match rect.size {
        Size::Physical(s) => (s.width as i32, s.height as i32),
        Size::Logical(s) => ((s.width * scale).round() as i32, (s.height * scale).round() as i32),
    };
    (x, y, w, h)
}

fn position_tray_panel_fallback(window: &WebviewWindow) {
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(400, 580));
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten();

    let Some(monitor) = monitor else {
        return;
    };

    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    #[cfg(target_os = "macos")]
    let pos = PhysicalPosition::new(
        mon_pos.x + mon_size.width as i32 - size.width as i32 - 12,
        mon_pos.y + 28,
    );

    #[cfg(not(target_os = "macos"))]
    let pos = PhysicalPosition::new(
        mon_pos.x + mon_size.width as i32 - size.width as i32 - 8,
        mon_pos.y + mon_size.height as i32 - size.height as i32 - 48,
    );

    let _ = window.set_position(pos);
}

