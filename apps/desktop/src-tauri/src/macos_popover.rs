//! macOS menubar helpers — activation policy + native NSPopover for tray history UI.

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use tauri::{ActivationPolicy, AppHandle, Emitter, Manager, WebviewWindow};
#[cfg(target_os = "macos")]
use tauri_plugin_nspopover::{AppExt, ToPopoverOptions, WindowExt};

#[cfg(target_os = "macos")]
static ACTIVATION_POLICY_FAILED: AtomicBool = AtomicBool::new(false);

/// LSUIElement in Info.plist already makes the app an accessory; this reinforces after Settings.
#[cfg(target_os = "macos")]
pub fn ensure_accessory_policy(app: &AppHandle) {
    if let Err(e) = app.set_activation_policy(ActivationPolicy::Accessory) {
        if !ACTIVATION_POLICY_FAILED.swap(true, Ordering::Relaxed) {
            tracing::warn!("popover: ActivationPolicy::Accessory failed ({e}); LSUIElement applies");
        }
    }
}

#[cfg(target_os = "macos")]
pub fn init_menubar_app_policy(_app: &AppHandle) {
    // Intentionally empty — LSUIElement in Info.plist; avoid redundant NSApplication calls at launch.
}

#[cfg(target_os = "macos")]
pub fn activate_settings_policy(app: &AppHandle) {
    hide_tray_nspopover(app);
    let _ = app.set_activation_policy(ActivationPolicy::Regular);
}

#[cfg(target_os = "macos")]
pub fn restore_menubar_app_policy(app: &AppHandle) {
    ensure_accessory_policy(app);
}

/// Convert the tray webview window into a native NSPopover anchored to the status item.
/// Requires tray icon id `"main"` (see `lib.rs` tray setup).
#[cfg(target_os = "macos")]
pub fn setup_tray_nspopover(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("tray") {
        window.to_popover(ToPopoverOptions {
            is_fullsize_content: true,
        });
        tracing::info!("tray window converted to native NSPopover");
    } else {
        tracing::warn!("tray window not found; NSPopover setup skipped");
    }
}

#[cfg(target_os = "macos")]
pub fn show_tray_nspopover(app: &AppHandle) {
    if !app.is_popover_shown() {
        // Reset visibility so TrayPanel refreshes after transient outside-dismiss.
        let _ = app.emit("tray-visibility", false);
        app.show_popover();
        let _ = app.emit("tray-visibility", true);
    }
}

#[cfg(target_os = "macos")]
pub fn hide_tray_nspopover(app: &AppHandle) {
    if app.is_popover_shown() {
        app.hide_popover();
        let _ = app.emit("tray-visibility", false);
    }
}

#[cfg(target_os = "macos")]
pub fn toggle_tray_nspopover(app: &AppHandle) {
    if app.is_popover_shown() {
        hide_tray_nspopover(app);
    } else {
        show_tray_nspopover(app);
    }
}

/// Quick Paste stays a Tauri overlay window (not NSPopover).
#[cfg(target_os = "macos")]
pub fn show_quick_paste_window(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);
    let _ = window.show();
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_accessory_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn init_menubar_app_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn activate_settings_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn restore_menubar_app_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn show_quick_paste_window(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    let _ = window.set_focus();
}
