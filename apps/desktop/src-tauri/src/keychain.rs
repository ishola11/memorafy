//! OS-native credential storage (Windows Credential Manager, macOS Keychain,
//! Linux Secret Service) for the Supabase auth session.
//!
//! Access and refresh tokens are long-lived credentials — storing them in
//! plaintext SQLite would let any process, backup tool, or disk-level access
//! read a user's cloud session. This module never panics: if no OS
//! credential backend is available (e.g. a headless Linux box with no
//! Secret Service), callers get an `Err` and the app degrades to
//! "signed out" rather than crashing.

use keyring::Error as KeyringError;

const SERVICE: &str = "com.memora.desktop";
const SESSION_ACCOUNT: &str = "auth_session";
/// Cached end-to-end encryption data key (see `crypto.rs`).
const DEK_ACCOUNT: &str = "sync_dek";

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| format!("keychain unavailable: {e}"))
}

fn store_named(account: &str, value: &str) -> Result<(), String> {
    entry(account)?
        .set_password(value)
        .map_err(|e| format!("could not save to the system keychain: {e}"))
}

/// `Ok(None)` when nothing is stored yet — a normal state, not an error.
fn load_named(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(format!("could not read from the system keychain: {e}")),
    }
}

/// Idempotent: clearing an already-empty entry is not an error.
fn clear_named(account: &str) -> Result<(), String> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not clear from the system keychain: {e}")),
    }
}

pub fn store(json: &str) -> Result<(), String> {
    store_named(SESSION_ACCOUNT, json)
}

pub fn load() -> Result<Option<String>, String> {
    load_named(SESSION_ACCOUNT)
}

pub fn clear() -> Result<(), String> {
    clear_named(SESSION_ACCOUNT)
}

pub fn store_dek(encoded: &str) -> Result<(), String> {
    store_named(DEK_ACCOUNT, encoded)
}

pub fn load_dek() -> Result<Option<String>, String> {
    load_named(DEK_ACCOUNT)
}

pub fn clear_dek() -> Result<(), String> {
    clear_named(DEK_ACCOUNT)
}
