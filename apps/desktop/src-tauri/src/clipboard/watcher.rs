use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use tauri::{AppHandle, Emitter, Manager};

use super::content::{classify, hash_content, CapturedContent};
use crate::AppState;

/// Watcher polls every 400ms; suppress enough iterations to cover programmatic writes.
const SUPPRESS_ITERATIONS: u32 = 6;

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
                if should_skip_capture(&state, &hash) {
                    last_hash = hash;
                    continue;
                }
                last_hash = hash.clone();

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
                if should_skip_capture(&state, &hash) {
                    last_hash = hash;
                    continue;
                }
                if let Err(e) = persist_image(&state, &app, &img, &mut last_hash) {
                    tracing::error!("persist image: {e}");
                } else {
                    let _ = app.emit("items-updated", ());
                }
            }
        }
    });
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
        &state.device_id,
        content_type,
        plain,
        url,
        None,
        None,
        None,
        hash,
    )?;
    state.db.touch_device(&state.device_id)?;
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

    // Save raw RGBA as simple PNG via minimal encoding - for MVP store raw bytes
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&image.bytes)?;

    let size = image.bytes.len() as i64;
    state.db.insert_item(
        &state.device_id,
        "image",
        None,
        None,
        Some(path.to_string_lossy().to_string()),
        Some(size),
        None,
        &hash,
    )?;
    state.db.touch_device(&state.device_id)?;

    let _ = app;
    Ok(())
}

pub fn write_clipboard(state: &AppState, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (content_type, _) = classify(text);
    let hash = hash_content(content_type, Some(text), None);
    {
        let mut suppress = state.suppress_clipboard.lock();
        *suppress = SUPPRESS_ITERATIONS;
        *state.last_programmatic_hash.lock() = Some(hash);
    }
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text.to_string())?;
    Ok(())
}

fn should_skip_capture(state: &AppState, hash: &str) -> bool {
    if state
        .last_programmatic_hash
        .lock()
        .as_deref()
        .is_some_and(|h| h == hash)
    {
        return true;
    }
    state
        .db
        .content_hash_exists(hash)
        .unwrap_or(false)
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
