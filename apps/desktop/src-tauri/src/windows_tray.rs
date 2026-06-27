//! Windows system-tray sidebar panel positioning and show/hide.

use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{Emitter, Manager, Monitor, PhysicalPosition, Position, Size, WebviewWindow};

use crate::macos_popover;

static TRAY_OPENED_AT: Mutex<Option<Instant>> = Mutex::new(None);
const TRAY_BLUR_GRACE: Duration = Duration::from_millis(450);

pub fn blur_grace_active() -> bool {
    TRAY_OPENED_AT
        .lock()
        .as_ref()
        .is_some_and(|t| t.elapsed() < TRAY_BLUR_GRACE)
}

pub fn open_tray(app: &tauri::AppHandle) {
    toggle_tray_window(app, true, None);
}

pub fn toggle_tray_window(
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
            macos_popover::show_quick_paste_window(&window);
            *TRAY_OPENED_AT.lock() = Some(Instant::now());
            let _ = app.emit("tray-visibility", true);
        } else {
            let _ = window.hide();
            let _ = app.emit("tray-visibility", false);
        }
    }
}

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

    let below_tray = tray_y + tray_h + gap;
    let above_taskbar = mon_pos.y + mon_size.height as i32 - size.height as i32 - 8;
    let y = if tray_y > mon_pos.y + mon_size.height as i32 / 2 {
        above_taskbar
    } else {
        below_tray
    };

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
    let monitor = window.primary_monitor().ok().flatten();

    let Some(monitor) = monitor else {
        return;
    };

    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let pos = PhysicalPosition::new(
        mon_pos.x + mon_size.width as i32 - size.width as i32 - 8,
        mon_pos.y + mon_size.height as i32 - size.height as i32 - 48,
    );

    let _ = window.set_position(pos);
}
