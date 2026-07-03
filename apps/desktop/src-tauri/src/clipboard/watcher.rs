use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use tauri::{AppHandle, Emitter, Manager};

use super::content::{classify, hash_content, hash_image, CapturedContent};
use super::images::{dimensions_label, make_thumbnail_png, save_png};
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

            if super::concealed::clipboard_is_concealed() {
                continue;
            }

            let text = clipboard.get_text().ok();
            let image = clipboard.get_image().ok();

            if let Some(ref img) = image {
                if should_capture_image(text.as_deref()) {
                    let hash = hash_image(img.width, img.height, &img.bytes);
                    if hash == last_hash {
                        continue;
                    }
                    if should_skip_capture(&state, &hash, None) {
                        last_hash = hash;
                        continue;
                    }
                    last_hash = hash.clone();
                    if let Err(e) = persist_image(&state, img, &hash) {
                        tracing::error!("persist image: {e}");
                    } else {
                        record_capture(&state, &hash);
                        let _ = app.emit("items-updated", ());
                        state.sync_engine.request_sync();
                    }
                    continue;
                }
            }

            if let Some(text) = text {
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
            }
        }
    });
}

/// macOS often puts a filename/path in plain text alongside real image bytes.
/// Prefer the image when both are present, or when the text is only a filename.
fn should_capture_image(text: Option<&str>) -> bool {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        None => true,
        Some(t) if text_looks_like_image_filename(t) => true,
        #[cfg(target_os = "macos")]
        Some(_) => true,
        #[cfg(not(target_os = "macos"))]
        Some(_) => false,
    }
}

fn text_looks_like_image_filename(text: &str) -> bool {
    if text.contains('\n') || text.len() > 260 {
        return false;
    }
    let lower = text.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".heic")
        || lower.ends_with(".tif")
        || lower.ends_with(".tiff")
        || lower.ends_with(".bmp")
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
    let width = image.width as u32;
    let height = image.height as u32;
    let id = uuid::Uuid::new_v4();
    let filename = format!("{id}.png");
    let thumb_name = format!("{id}_thumb.png");
    let path = state.db.blobs_dir().join(&filename);
    let thumb_path = state.db.blobs_dir().join(&thumb_name);

    save_png(&path, width, height, &image.bytes)?;
    let thumb_bytes = make_thumbnail_png(width, height, &image.bytes)?;
    std::fs::write(&thumb_path, thumb_bytes)?;

    let size = std::fs::metadata(&path)?.len() as i64;
    let label = dimensions_label(width, height);
    state.db.insert_image_item(
        &state.device_id(),
        path.to_string_lossy().into_owned(),
        size,
        thumb_path.to_string_lossy().into_owned(),
        &label,
        hash,
    )?;
    state.db.touch_device(&state.device_id())?;
    Ok(())
}

pub fn write_clipboard(state: &AppState, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    write_clipboard_internal(state, text, None)
}

pub fn write_clipboard_rich(
    state: &AppState,
    plain_text: &str,
    html: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    write_clipboard_internal(state, plain_text, Some(html))
}

pub fn write_clipboard_image(
    state: &AppState,
    blob_path: &std::path::Path,
    dimensions_label: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::borrow::Cow;

    let dims = dimensions_label.and_then(super::images::parse_dimensions_label);
    let decoded = super::images::load_image_blob(blob_path, dims)?;
    let hash = hash_image(
        decoded.width as usize,
        decoded.height as usize,
        &decoded.rgba,
    );
    {
        let mut suppress = state.suppress_clipboard.lock();
        *suppress = SUPPRESS_ITERATIONS;
        *state.last_programmatic_hash.lock() = Some(hash.clone());
        *state.last_capture.lock() = Some((hash, Instant::now()));
    }
    let mut clipboard = Clipboard::new()?;
    clipboard.set_image(arboard::ImageData {
        width: decoded.width as usize,
        height: decoded.height as usize,
        bytes: Cow::Owned(decoded.rgba),
    })?;
    Ok(())
}

fn write_clipboard_internal(
    state: &AppState,
    plain_text: &str,
    html: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (content_type, _) = classify(plain_text);
    let hash = hash_content(content_type, Some(plain_text), None);
    {
        let mut suppress = state.suppress_clipboard.lock();
        *suppress = SUPPRESS_ITERATIONS;
        *state.last_programmatic_hash.lock() = Some(hash.clone());
        *state.last_capture.lock() = Some((hash, Instant::now()));
    }
    let mut clipboard = Clipboard::new()?;
    match html {
        Some(html) => clipboard.set_html(html, Some(plain_text))?,
        None => clipboard.set_text(plain_text.to_string())?,
    }
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
