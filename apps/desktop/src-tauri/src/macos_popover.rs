#![allow(deprecated)]

//! macOS menubar popover configuration for tray + quick-paste overlays.
//!
//! **Never wrap `msg_send!` in `catch_unwind`** — AppKit exceptions are foreign and abort the
//! process ("Rust cannot catch foreign exceptions"). Only use NSWindow-safe selectors here.

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use tauri::{ActivationPolicy, AppHandle, WebviewWindow};

#[cfg(target_os = "macos")]
static ACTIVATION_POLICY_FAILED: AtomicBool = AtomicBool::new(false);

/// Menubar apps must be Accessory so they can overlay another app's full-screen Space.
#[cfg(target_os = "macos")]
pub fn ensure_accessory_policy(app: &AppHandle) {
    if let Err(e) = app.set_activation_policy(ActivationPolicy::Accessory) {
        if !ACTIVATION_POLICY_FAILED.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                "popover: could not set ActivationPolicy::Accessory ({e}); LSUIElement in Info.plist still applies"
            );
        }
    }
}

#[cfg(target_os = "macos")]
pub fn init_menubar_app_policy(app: &AppHandle) {
    ensure_accessory_policy(app);
}

#[cfg(target_os = "macos")]
pub fn activate_settings_policy(app: &AppHandle) {
    let _ = app.set_activation_policy(ActivationPolicy::Regular);
}

#[cfg(target_os = "macos")]
pub fn restore_menubar_app_policy(app: &AppHandle) {
    ensure_accessory_policy(app);
}

#[cfg(target_os = "macos")]
fn ns_window_handle(window: &WebviewWindow) -> Option<cocoa::base::id> {
    let Ok(raw) = window.ns_window() else {
        return None;
    };
    let ns_win = raw as cocoa::base::id;
    if ns_win.is_null() {
        return None;
    }
    Some(ns_win)
}

/// NSWindow-only AppKit flags (no NSPanel selectors — those throw on NSWindow).
#[cfg(target_os = "macos")]
unsafe fn apply_popover_ns_config(ns_win: cocoa::base::id) {
    use cocoa::appkit::NSWindowCollectionBehavior;
    use cocoa::base::YES;
    use objc::{msg_send, sel, sel_impl};

    const POPUP_MENU_WINDOW_LEVEL: i64 = 101;

    let existing: NSWindowCollectionBehavior = msg_send![ns_win, collectionBehavior];
    let behavior = existing
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
    let _: () = msg_send![ns_win, setCollectionBehavior: behavior];
    let _: () = msg_send![ns_win, setLevel: POPUP_MENU_WINDOW_LEVEL];

    if msg_send![ns_win, respondsToSelector: sel!(setCanAppearWhileOtherAppIsFullScreen:)] {
        let _: () = msg_send![ns_win, setCanAppearWhileOtherAppIsFullScreen: YES];
    }
}

#[cfg(target_os = "macos")]
pub fn configure_popover_window(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);

    let Some(ns_win) = ns_window_handle(window) else {
        tracing::debug!("popover: NSWindow handle not ready");
        return;
    };

    unsafe {
        apply_popover_ns_config(ns_win);
    }
}

/// Show overlay on the active Space without activating the app (no Space switch).
#[cfg(target_os = "macos")]
pub fn show_popover_window(app: &AppHandle, window: &WebviewWindow) {
    use objc::{msg_send, sel, sel_impl};

    ensure_accessory_policy(app);
    configure_popover_window(window);

    let Some(ns_win) = ns_window_handle(window) else {
        let _ = window.show();
        return;
    };

    unsafe {
        let _: () = msg_send![ns_win, orderFrontRegardless];
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

#[cfg(not(target_os = "macos"))]
pub fn configure_popover_window(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(true);
}

#[cfg(not(target_os = "macos"))]
pub fn show_popover_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let _ = app;
    let _ = window.show();
    let _ = window.set_focus();
}
