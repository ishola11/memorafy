//! macOS menubar helpers — activation policy + native NSPopover for tray history UI.

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use tauri::{ActivationPolicy, AppHandle, Emitter, Manager};
#[cfg(target_os = "macos")]
use tauri_plugin_nspopover::{AppExt, ToPopoverOptions, WindowExt};

#[cfg(target_os = "macos")]
static ACTIVATION_POLICY_FAILED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static POPOVER_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Set Accessory policy at launch so the app behaves as a menubar-only utility (Parallel/Paste style).
#[cfg(target_os = "macos")]
pub fn init_menubar_app_policy(app: &AppHandle) {
    ensure_accessory_policy(app);
}

#[cfg(target_os = "macos")]
pub fn ensure_accessory_policy(app: &AppHandle) {
    if let Err(e) = app.set_activation_policy(ActivationPolicy::Accessory) {
        if !ACTIVATION_POLICY_FAILED.swap(true, Ordering::Relaxed) {
            tracing::warn!("popover: ActivationPolicy::Accessory failed ({e}); LSUIElement applies");
        }
    }
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

/// Convert the tray webview window into a native NSPopover anchored to status item `"main"`.
/// Requires `trayIcon.id = "main"` in `tauri.macos.conf.json` (not TrayIconBuilder).
#[cfg(target_os = "macos")]
pub fn setup_tray_nspopover(app: &AppHandle) -> bool {
    if POPOVER_INITIALIZED.load(Ordering::Relaxed) {
        return true;
    }

    if app.tray_by_id("main").is_none() {
        tracing::warn!("popover: tray id 'main' not found — deferred (use tauri.macos.conf.json trayIcon)");
        return false;
    }

    let Some(window) = app.get_webview_window("tray") else {
        tracing::warn!("popover: tray webview window not found — deferred");
        return false;
    };

    window.to_popover(ToPopoverOptions {
        is_fullsize_content: true,
    });
    POPOVER_INITIALIZED.store(true, Ordering::Relaxed);
    tracing::info!("popover: tray window converted to native NSPopover");
    true
}

/// Retry popover setup on `RunEvent::Ready` when the tray webview wasn't ready during `setup()`.
#[cfg(target_os = "macos")]
pub fn retry_setup_tray_nspopover(app: &AppHandle) {
    if POPOVER_INITIALIZED.load(Ordering::Relaxed) {
        return;
    }
    if setup_tray_nspopover(app) {
        tracing::info!("popover: initialized on Ready");
    } else {
        tracing::warn!("popover: init still pending after Ready — left-click will retry");
    }
}

#[cfg(target_os = "macos")]
fn ensure_popover_ready(app: &AppHandle) -> bool {
    if POPOVER_INITIALIZED.load(Ordering::Relaxed) {
        return true;
    }
    tracing::warn!("popover: not initialized on click — retrying setup");
    setup_tray_nspopover(app)
}

#[cfg(target_os = "macos")]
pub fn show_tray_nspopover(app: &AppHandle) {
    if !ensure_popover_ready(app) {
        tracing::error!("popover: cannot show — setup failed (tray 'main' or tray window missing)");
        return;
    }

    if !app.is_popover_shown() {
        let _ = app.emit("tray-visibility", false);
        app.show_popover();
        let _ = app.emit("tray-visibility", true);
    }
}

#[cfg(target_os = "macos")]
pub fn hide_tray_nspopover(app: &AppHandle) {
    if !POPOVER_INITIALIZED.load(Ordering::Relaxed) {
        return;
    }
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

#[cfg(not(target_os = "macos"))]
pub fn ensure_accessory_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn init_menubar_app_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn activate_settings_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn restore_menubar_app_policy(_app: &tauri::AppHandle) {}
