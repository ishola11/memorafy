#![allow(deprecated)]

//! macOS menubar popover — Tauri window overlay path.
//!
//! Native `msg_send!` AppKit configuration was removed: selectors like
//! `setCollectionBehavior:` / `setCanAppearWhileOtherAppIsFullScreen:` throw NSExceptions
//! on Tauri's WKWebView `NSWindow`, which aborts the process if any frame uses `catch_unwind`
//! (including Tauri/wry internals) — "Rust cannot catch foreign exceptions".
//!
//! `LSUIElement` in Info.plist + Tauri `alwaysOnTop` / `visibleOnAllWorkspaces` is the safe baseline.
//! Full-screen Space overlay may require Option B (native NSPopover) in a future phase.

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use tauri::{ActivationPolicy, AppHandle, WebviewWindow};

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
    let _ = app.set_activation_policy(ActivationPolicy::Regular);
}

#[cfg(target_os = "macos")]
pub fn restore_menubar_app_policy(app: &AppHandle) {
    ensure_accessory_policy(app);
}

#[cfg(target_os = "macos")]
pub fn configure_popover_window(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.set_visible_on_all_workspaces(true);
}

/// Show tray / quick-paste overlay without native AppKit selectors.
#[cfg(target_os = "macos")]
pub fn show_popover_window(_app: &AppHandle, window: &WebviewWindow) {
    configure_popover_window(window);
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
pub fn configure_popover_window(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(true);
}

#[cfg(not(target_os = "macos"))]
pub fn show_popover_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let _ = app;
    let _ = window.show();
    let _ = window.set_focus();
}
