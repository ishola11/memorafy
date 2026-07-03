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
const ACCOUNT: &str = "auth_session";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("keychain unavailable: {e}"))
}

/// Stores the serialized session, overwriting any existing entry.
pub fn store(json: &str) -> Result<(), String> {
    entry()?
        .set_password(json)
        .map_err(|e| format!("could not save session to the system keychain: {e}"))
}

/// `Ok(None)` when nothing is stored yet — a normal signed-out state, not an error.
pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(json) => Ok(Some(json)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(format!("could not read session from the system keychain: {e}")),
    }
}

/// Idempotent: clearing an already-empty entry is not an error.
pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not clear session from the system keychain: {e}")),
    }
}
