//! macOS menubar popover configuration for tray + quick-paste overlays.
//!
//! Tried in Phase 3.1: `alwaysOnTop` + `visibleOnAllWorkspaces` + `NSMainMenuWindowLevel+1`
//! — insufficient; panel still landed on primary Space / behind fullscreen apps.
//!
//! Native menubar apps (Paste, Raycast, Parallel) use NSStatusItem + a non-activating
//! panel at `NSPopUpMenuWindowLevel` with `CanJoinAllSpaces | Stationary |
//! FullScreenAuxiliary`. Regular `NSWindow` cannot render over fullscreen Spaces unless
//! the app uses `ActivationPolicy::Accessory` (LSUIElement) and the above flags are OR'd
//! into the existing `collectionBehavior` (not overwritten). See tauri#11488, tauri-nspanel.
//!
//! Tauri webview windows are `NSWindow`, not `NSPanel` — do not call NSPanel-only selectors
//! (`setFloatingPanel:`, `setWorksWhenModal:`, `setHidesOnDeactivate:`) or AppKit raises
//! an exception that aborts the process (non-unwinding panic).

#![allow(unexpected_cfgs, deprecated)]

#[cfg(target_os = "macos")]
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use tauri::{ActivationPolicy, AppHandle, WebviewWindow};

#[cfg(target_os = "macos")]
static ACTIVATION_POLICY_FAILED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "macos")]
static POPOVER_CFG_FAILED: AtomicBool = AtomicBool::new(false);

/// Menubar apps should not appear in the Dock; required for `CanJoinAllSpaces` over fullscreen.
/// Call after the event loop is running (e.g. `RunEvent::Ready`), not during `setup`.
#[cfg(target_os = "macos")]
pub fn init_menubar_app_policy(app: &AppHandle) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        app.set_activation_policy(ActivationPolicy::Accessory)
    }));
    if result.is_err() || result.as_ref().ok().and_then(|r| r.as_ref().err()).is_some() {
        if !ACTIVATION_POLICY_FAILED.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                "popover: could not set ActivationPolicy::Accessory; LSUIElement in Info.plist still applies"
            );
        }
    }
}

/// Switch to Regular when opening a normal settings window (shows Dock icon while settings open).
#[cfg(target_os = "macos")]
pub fn activate_settings_policy(app: &AppHandle) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        app.set_activation_policy(ActivationPolicy::Regular)
    }));
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

/// NSWindow-safe popover flags (no NSPanel-only selectors).
#[cfg(target_os = "macos")]
unsafe fn apply_popover_ns_config(ns_win: cocoa::base::id) {
    use cocoa::appkit::NSWindowCollectionBehavior;
    use objc::{msg_send, sel, sel_impl};

    // NSPopUpMenuWindowLevel — same tier as native menu-bar extras / popovers.
    const POPUP_MENU_WINDOW_LEVEL: i64 = 101;
    // NSWindowStyleMaskNonactivatingPanel — panel receives clicks without activating the app.
    const NONACTIVATING_PANEL: usize = 1 << 7;

    let existing: NSWindowCollectionBehavior = msg_send![ns_win, collectionBehavior];
    let behavior = existing
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
    let _: () = msg_send![ns_win, setCollectionBehavior: behavior];

    let _: () = msg_send![ns_win, setLevel: POPUP_MENU_WINDOW_LEVEL];

    let style_mask: usize = msg_send![ns_win, styleMask];
    if style_mask & NONACTIVATING_PANEL == 0 {
        let _: () = msg_send![ns_win, setStyleMask: style_mask | NONACTIVATING_PANEL];
    }
}

#[cfg(target_os = "macos")]
pub fn configure_popover_window(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);

    let Some(ns_win) = ns_window_handle(window) else {
        tracing::debug!("popover: NSWindow handle not ready, deferring native config");
        return;
    };

    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        apply_popover_ns_config(ns_win);
    }));

    if result.is_err() {
        if !POPOVER_CFG_FAILED.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                "popover: native NSWindow configuration failed; overlay may not appear over fullscreen Spaces"
            );
        }
    }
}

/// Show overlay above fullscreen Spaces without moving it to the primary desktop.
#[cfg(target_os = "macos")]
pub fn show_popover_window(window: &WebviewWindow) {
    use objc::{msg_send, sel, sel_impl};

    configure_popover_window(window);

    let Some(ns_win) = ns_window_handle(window) else {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    };

    let ordered = catch_unwind(AssertUnwindSafe(|| unsafe {
        let _: () = msg_send![ns_win, orderFrontRegardless];
    }));

    if ordered.is_err() {
        let _ = window.show();
    }
    let _ = window.set_focus();
}

#[cfg(not(target_os = "macos"))]
pub fn init_menubar_app_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn activate_settings_policy(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn configure_popover_window(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(true);
}

#[cfg(not(target_os = "macos"))]
pub fn show_popover_window(window: &tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}
