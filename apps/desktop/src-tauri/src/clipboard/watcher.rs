use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use tauri::{AppHandle, Emitter, Manager};

use super::content::{classify, hash_content, hash_image, CapturedContent};
use crate::AppState;

/// Watcher polls every 400ms; suppress enough iterations to cover programmatic writes.
const SUPPRESS_ITERATIONS: u32 = 20;
const RAPID_CAPTURE_WINDOW: Duration = Duration::from_secs(30);
const RECENT_PLAIN_TEXT_LIMIT: i64 = 8;
/// Skip pathological captures (multi-hundred-MB selections) that would bloat
/// SQLite and stall the UI. The live clipboard is untouched — we just don't
/// record the item in history.
const MAX_TEXT_CAPTURE_BYTES: usize = 10 * 1024 * 1024;
const CLIPBOARD_INIT_RETRY: Duration = Duration::from_secs(5);

pub fn start_watcher(app: AppHandle) {
    thread::spawn(move || {
        // The clipboard can be transiently unavailable at startup (another
        // process holding it, session still initializing). Keep retrying —
        // a clipboard manager with a dead watcher is silently useless.
        let mut clipboard = loop {
            match Clipboard::new() {
                Ok(c) => break c,
                Err(e) => {
                    tracing::warn!(
                        "clipboard unavailable, retrying in {}s: {e}",
                        CLIPBOARD_INIT_RETRY.as_secs()
                    );
                    thread::sleep(CLIPBOARD_INIT_RETRY);
                }
            }
        };
        tracing::info!("clipboard watcher started");

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
                if text.trim().is_empty() {
                    continue;
                }
                if text.len() > MAX_TEXT_CAPTURE_BYTES {
                    tracing::info!(
                        "skipping oversized clipboard text ({} bytes > {} max)",
                        text.len(),
                        MAX_TEXT_CAPTURE_BYTES
                    );
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
                    state.sync_engine.request_sync();
                }
            } else if let Ok(img) = clipboard.get_image() {
                let hash = hash_image(img.width, img.height, &img.bytes);
                if hash == last_hash {
                    continue;
                }
                if should_skip_capture(&state, &hash, None) {
                    last_hash = hash;
                    continue;
                }
                last_hash = hash.clone();
                if let Err(e) = persist_image(&state, &img, &hash) {
                    tracing::error!("persist image: {e}");
                } else {
                    record_capture(&state, &hash);
                    let _ = app.emit("items-updated", ());
                    state.sync_engine.request_sync();
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
    image: &arboard::ImageData,
    hash: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

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
        hash,
    )?;
    state.db.touch_device(&state.device_id())?;
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

    false
}
