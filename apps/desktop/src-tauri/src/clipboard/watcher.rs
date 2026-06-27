use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use tauri::{AppHandle, Emitter, Manager};

use super::content::{classify, hash_content, CapturedContent};
use crate::AppState;

/// Watcher polls every 400ms; suppress enough iterations to cover programmatic writes.
const SUPPRESS_ITERATIONS: u32 = 20;
const DEDUPE_WINDOW: Duration = Duration::from_secs(300);
const RAPID_CAPTURE_WINDOW: Duration = Duration::from_secs(30);
const RECENT_PLAIN_TEXT_LIMIT: i64 = 8;

pub fn start_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("clipboard init failed: {e}");
                return;
            }
        };

        let mut last_hash = String::new();

        loop {
            thread::sleep(Duration::from_millis(400));

            let state = match app.try_state::<AppState>() {
                Some(s) => s,
                None => continue,
            };

            if state.clipboard_paused.load(Ordering::Relaxed) {
                continue;
            }

            {
                let mut suppress = state.suppress_clipboard.lock();
                if *suppress > 0 {
                    *suppress -= 1;
                    continue;
                }
            }

            if let Ok(text) = clipboard.get_text() {
                if text.is_empty() || should_ignore_clipboard_text(&text) {
                    continue;
                }
                let (content_type, captured) = classify(&text);
                let hash = hash_content(content_type, Some(&text), None);
                if hash == last_hash {
                    continue;
                }
                if should_skip_capture(&state, &hash, Some(&text)) {
                    last_hash = hash;
                    continue;
                }
                last_hash = hash.clone();
                record_capture(&state, &hash);

                if let Err(e) = persist_capture(&state, content_type, captured, &hash) {
                    tracing::error!("persist capture: {e}");
                } else {
                    let _ = app.emit("items-updated", ());
                }
            } else if let Ok(img) = clipboard.get_image() {
                let hash = hash_content("image", None, Some(&format!("{}x{}", img.width, img.height)));
                if hash == last_hash {
                    continue;
                }
                if should_skip_capture(&state, &hash, None) {
                    last_hash = hash;
                    continue;
                }
                if let Err(e) = persist_image(&state, &app, &img, &mut last_hash) {
                    tracing::error!("persist image: {e}");
                } else {
                    record_capture(&state, &hash);
                    let _ = app.emit("items-updated", ());
                }
            }
        }
    });
}

fn record_capture(state: &AppState, hash: &str) {
    *state.last_capture.lock() = Some((hash.to_string(), Instant::now()));
}

fn persist_capture(
    state: &AppState,
    content_type: &str,
    captured: CapturedContent,
    hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (plain, url) = match captured {
        CapturedContent::Text(t) => (Some(t), None),
        CapturedContent::Url(u) => (Some(u.clone()), Some(u)),
        CapturedContent::Code(c) => (Some(c), None),
        CapturedContent::Image { .. } => (None, None),
    };

    state.db.insert_item(
        &state.device_id(),
        content_type,
        plain,
        url,
        None,
        None,
        None,
        hash,
    )?;
    state.db.touch_device(&state.device_id())?;
    Ok(())
}

fn persist_image(
    state: &AppState,
    app: &AppHandle,
    image: &arboard::ImageData,
    last_hash: &mut String,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let hash = hash_content("image", None, Some(&format!("{}x{}", image.width, image.height)));
    if hash == *last_hash {
        return Ok(());
    }
    *last_hash = hash.clone();

    let filename = format!("{}.png", uuid::Uuid::new_v4());
    let path = state.db.blobs_dir().join(&filename);

    let mut file = std::fs::File::create(&path)?;
    file.write_all(&image.bytes)?;

    let size = image.bytes.len() as i64;
    state.db.insert_item(
        &state.device_id(),
        "image",
        None,
        None,
        Some(path.to_string_lossy().to_string()),
        Some(size),
        None,
        &hash,
    )?;
    state.db.touch_device(&state.device_id())?;

    let _ = app;
    Ok(())
}

pub fn write_clipboard(state: &AppState, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (content_type, _) = classify(text);
    let hash = hash_content(content_type, Some(text), None);
    {
        let mut suppress = state.suppress_clipboard.lock();
        *suppress = SUPPRESS_ITERATIONS;
        *state.last_programmatic_hash.lock() = Some(hash.clone());
        *state.last_capture.lock() = Some((hash, Instant::now()));
    }
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text.to_string())?;
    Ok(())
}

fn should_skip_capture(state: &AppState, hash: &str, plain_text: Option<&str>) -> bool {
    if state
        .last_programmatic_hash
        .lock()
        .as_deref()
        .is_some_and(|h| h == hash)
    {
        return true;
    }

    if let Some((last_hash, at)) = state.last_capture.lock().clone() {
        if last_hash == hash && at.elapsed() < RAPID_CAPTURE_WINDOW {
            return true;
        }
    }

    if state.db.content_hash_exists(hash).unwrap_or(false) {
        return true;
    }

    if state.db.content_hash_synced_exists(hash).unwrap_or(false) {
        return true;
    }

    if let Some(text) = plain_text {
        if state
            .db
            .recent_plain_text_exists(text, RECENT_PLAIN_TEXT_LIMIT)
            .unwrap_or(false)
        {
            return true;
        }
    }

    let _ = DEDUPE_WINDOW;
    false
}

/// Skip terminal output, compiler warnings, and other non-user clipboard noise.
fn should_ignore_clipboard_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 3 {
        return true;
    }
    let noise_markers = [
        "warning:",
        "error:",
        "Finished `dev` profile",
        "Finished `release` profile",
        "Compiling memora-desktop",
        "Running `target/",
        "--> src",
        "--> src\\",
        "FOREIGN KEY constraint",
        "memora_desktop_lib::",
    ];
    noise_markers.iter().any(|m| trimmed.contains(m))
}
