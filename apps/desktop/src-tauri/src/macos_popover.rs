#![allow(deprecated)]

//! macOS menubar popover configuration for tray + quick-paste overlays.
//!
//! ## Root cause (fullscreen Spaces)
//! Full-screen apps each occupy their own Space. A normal `NSWindow` tied to the desktop
//! Space will not appear over another app's full-screen window. Overlay requires:
//! - `ActivationPolicy::Accessory` / `LSUIElement` (not Regular/Dock app)
//! - `NSWindowCollectionBehaviorFullScreenAuxiliary` + `CanJoinAllSpaces` / `MoveToActiveSpace`
//! - High window level (`NSPopUpMenuWindowLevel`)
//! - `canAppearWhileOtherAppIsFullScreen = YES` when supported
//! - **No** `NSWindowCollectionBehaviorStationary` — that pins the window to the Space
//!   where it was first shown (desktop/home).
//! - **No** `makeKeyAndOrderFront` / `set_focus` on show — that can switch Spaces.
//!
//! ## Option chosen: A (Tauri webview window + AppKit flags)
//! Keeps existing React UI. Option B (`NSStatusItem` + `NSPopover`) is more native but
//! requires hosting web content in an embedded view or rewriting the panel — larger effort
//! for marginal gain if the collection/level flags are applied correctly.
//!
//! NSPanel-only selectors are guarded with `respondsToSelector:` + `catch_unwind`.

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

/// Menubar apps must be Accessory so they can overlay another app's full-screen Space.
#[cfg(target_os = "macos")]
pub fn ensure_accessory_policy(app: &AppHandle) {
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

/// Call once after the event loop starts.
#[cfg(target_os = "macos")]
pub fn init_menubar_app_policy(app: &AppHandle) {
    ensure_accessory_policy(app);
}

/// Switch to Regular when opening Settings (Dock icon while settings is open).
#[cfg(target_os = "macos")]
pub fn activate_settings_policy(app: &AppHandle) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        app.set_activation_policy(ActivationPolicy::Regular)
    }));
}

/// Restore Accessory after Settings closes — Regular policy breaks full-screen overlay.
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

#[cfg(target_os = "macos")]
unsafe fn send_bool_if_responds(ns_win: cocoa::base::id, selector: objc::runtime::Sel, value: bool) {
    use cocoa::base::{NO, YES};
    use objc::{msg_send, sel, sel_impl};

    if msg_send![ns_win, respondsToSelector: selector] {
        let flag = if value { YES } else { NO };
        let _: () = msg_send![ns_win, selector, flag];
    }
}

/// AppKit overlay flags for full-screen Space compatibility.
#[cfg(target_os = "macos")]
unsafe fn apply_popover_ns_config(ns_win: cocoa::base::id) {
    use cocoa::appkit::NSWindowCollectionBehavior;
    use objc::{msg_send, sel, sel_impl};

    // NSPopUpMenuWindowLevel — menubar popover tier (above full-screen content).
    const POPUP_MENU_WINDOW_LEVEL: i64 = 101;
    const NONACTIVATING_PANEL: usize = 1 << 7;

    let existing: NSWindowCollectionBehavior = msg_send![ns_win, collectionBehavior];
    // MoveToActiveSpace: follow the Space the user is in (not pinned to desktop).
    // FullScreenAuxiliary: allowed in another app's full-screen Space.
    // CanJoinAllSpaces: visible on every Space.
    // Do NOT set Stationary — pins window to the Space where it was first created.
    let behavior = existing
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
    let _: () = msg_send![ns_win, setCollectionBehavior: behavior];

    let _: () = msg_send![ns_win, setLevel: POPUP_MENU_WINDOW_LEVEL];

    // macOS 10.11+ — explicit permission to appear over other apps' full-screen windows.
    send_bool_if_responds(
        ns_win,
        sel!(setCanAppearWhileOtherAppIsFullScreen:),
        true,
    );

    // NSPanel-only — safe no-op on NSWindow when selector is absent.
    send_bool_if_responds(ns_win, sel!(setFloatingPanel:), true);
    send_bool_if_responds(ns_win, sel!(setHidesOnDeactivate:), false);

    let style_result = catch_unwind(AssertUnwindSafe(|| {
        let style_mask: usize = msg_send![ns_win, styleMask];
        if style_mask & NONACTIVATING_PANEL == 0 {
            let _: () = msg_send![ns_win, setStyleMask: style_mask | NONACTIVATING_PANEL];
        }
    }));
    if style_result.is_err() {
        tracing::debug!("popover: non-activating styleMask not supported on this window class");
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

/// Show overlay on the active Space without switching away from a full-screen app.
#[cfg(target_os = "macos")]
pub fn show_popover_window(app: &AppHandle, window: &WebviewWindow) {
    use objc::{msg_send, sel, sel_impl};

    ensure_accessory_policy(app);
    configure_popover_window(window);

    let Some(ns_win) = ns_window_handle(window) else {
        let _ = window.show();
        return;
    };

    let ordered = catch_unwind(AssertUnwindSafe(|| unsafe {
        let _: () = msg_send![ns_win, orderFrontRegardless];
    }));

    if ordered.is_err() {
        let _ = window.show();
    }
    // Do NOT call set_focus / makeKeyAndOrderFront — activates app and can switch Spaces.
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
