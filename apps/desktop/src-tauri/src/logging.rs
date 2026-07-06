//! Production logging: daily-rotating files in the app data dir, plus a
//! panic hook so aborts (release uses `panic = "abort"`) leave a trace.

use std::fs;
use std::path::PathBuf;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Rotated log files kept on disk (one per day).
const MAX_LOG_FILES: usize = 7;
const LOG_FILE_PREFIX: &str = "memorafy.log";

/// Matches Tauri's `app_data_dir` for our identifier without needing an
/// `AppHandle`, so logging can start before the app is built.
pub fn app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("com.memorafy.app"))
}

pub fn log_dir() -> Option<PathBuf> {
    app_data_dir().map(|d| d.join("logs"))
}

/// Initialize file + console logging and the panic hook. Never panics:
/// if the log directory is unavailable we fall back to console-only.
pub fn init() {
    let filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,hyper=warn,reqwest=warn,tungstenite=warn,tokio_tungstenite=warn")
        })
    };

    let file_dir = log_dir().and_then(|dir| fs::create_dir_all(&dir).ok().map(|_| dir));

    match file_dir {
        Some(dir) => {
            prune_old_logs(&dir);
            // Synchronous writer (no `non_blocking`): log volume is low and a
            // panic followed by abort must not lose the final error line.
            let file_appender = tracing_appender::rolling::daily(&dir, LOG_FILE_PREFIX);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(true);
            let console_layer = tracing_subscriber::fmt::layer().with_target(true);

            if tracing_subscriber::registry()
                .with(filter())
                .with(file_layer)
                .with(console_layer)
                .try_init()
                .is_err()
            {
                eprintln!("memorafy: logging already initialized");
            }
        }
        None => {
            if tracing_subscriber::registry()
                .with(filter())
                .with(tracing_subscriber::fmt::layer().with_target(true))
                .try_init()
                .is_err()
            {
                eprintln!("memorafy: logging already initialized");
            }
            tracing::warn!("log directory unavailable — file logging disabled");
        }
    }

    install_panic_hook();
}

/// Log panics before the process aborts, keeping the default stderr output.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_string());
        tracing::error!(target: "panic", %location, "panic: {message}");
        default_hook(info);
    }));
}

/// Keep only the newest `MAX_LOG_FILES` rotated log files.
fn prune_old_logs(dir: &std::path::Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(LOG_FILE_PREFIX))
        })
        .collect();
    if logs.len() <= MAX_LOG_FILES {
        return;
    }
    // Daily-rotation filenames sort chronologically (memorafy.log.YYYY-MM-DD).
    logs.sort();
    let excess = logs.len() - MAX_LOG_FILES;
    for path in logs.into_iter().take(excess) {
        if let Err(e) = fs::remove_file(&path) {
            tracing::debug!("prune log {}: {e}", path.display());
        }
    }
}

/// Last `max_lines` lines of today's log file, newest last. Used by the
/// Feedback diagnostics preview so users see exactly what would be sent.
pub fn recent_log_tail(max_lines: usize) -> Option<String> {
    let dir = log_dir()?;
    let mut logs: Vec<PathBuf> = fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(LOG_FILE_PREFIX))
        })
        .collect();
    logs.sort();
    let newest = logs.pop()?;
    let content = fs::read_to_string(&newest).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Some(lines[start..].join("\n"))
}
