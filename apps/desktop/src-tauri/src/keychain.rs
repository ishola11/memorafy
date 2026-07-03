//! OS-native credential storage (Windows Credential Manager, macOS Keychain,
//! Linux Secret Service) for the Supabase auth session and cached encryption key.
//!
//! In **debug builds** (local `cargo run` / dev), secrets stay in the local
//! SQLite database instead of the OS keychain. Unsigned macOS debug binaries
//! trigger a scary "login keychain password" dialog on every access; dev mode
//! avoids that. Set `MEMORA_LOCAL_SECRETS=1` to force local storage in release
//! builds too (e.g. CI smoke tests).
//!
//! Release builds use the keychain with the human-readable service name "Memora"
//! so Keychain Access shows a friendly label instead of the bundle identifier.

use keyring::Error as KeyringError;

/// Shown in macOS Keychain Access and Windows Credential Manager.
const SERVICE: &str = "Memora";
const SESSION_ACCOUNT: &str = "auth_session";
const DEK_ACCOUNT: &str = "sync_dek";

/// When true, callers should store secrets in SQLite instead of the OS keychain.
pub fn prefer_local_storage() -> bool {
    std::env::var("MEMORA_LOCAL_SECRETS").is_ok() || cfg!(debug_assertions)
}

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| format!("keychain unavailable: {e}"))
}

fn store_named(account: &str, value: &str) -> Result<(), String> {
    entry(account)?
        .set_password(value)
        .map_err(|e| format!("could not save to the system keychain: {e}"))
}

fn load_named(account: &str) -> Result<Option<String>, String> {
    match entry(account)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(format!("could not read from the system keychain: {e}")),
    }
}

fn clear_named(account: &str) -> Result<(), String> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not clear from the system keychain: {e}")),
    }
}

pub fn store(json: &str) -> Result<(), String> {
    if prefer_local_storage() {
        return Err("local storage preferred".into());
    }
    store_named(SESSION_ACCOUNT, json)
}

pub fn load() -> Result<Option<String>, String> {
    if prefer_local_storage() {
        return Err("local storage preferred".into());
    }
    load_named(SESSION_ACCOUNT)
}

pub fn clear() -> Result<(), String> {
    if prefer_local_storage() {
        return Err("local storage preferred".into());
    }
    clear_named(SESSION_ACCOUNT)
}

pub fn store_dek(encoded: &str) -> Result<(), String> {
    if prefer_local_storage() {
        return Err("local storage preferred".into());
    }
    store_named(DEK_ACCOUNT, encoded)
}

pub fn load_dek() -> Result<Option<String>, String> {
    if prefer_local_storage() {
        return Err("local storage preferred".into());
    }
    load_named(DEK_ACCOUNT)
}

pub fn clear_dek() -> Result<(), String> {
    if prefer_local_storage() {
        return Err("local storage preferred".into());
    }
    clear_named(DEK_ACCOUNT)
}
