//! End-to-end encryption for synced clipboard content.
//!
//! Scheme:
//! - A random 256-bit **data key (DEK)** encrypts item fields with
//!   XChaCha20-Poly1305 (random 24-byte nonce per value).
//! - The DEK is **wrapped** (encrypted) with a **key-encryption key (KEK)**
//!   derived from the user's password via Argon2id, salted with the user id.
//!   Only the wrapped blob is stored server-side — Supabase never holds
//!   anything that can decrypt clipboard content.
//! - Cloud `content_hash` values are HMAC-SHA256 under a key derived from
//!   the DEK, so the server cannot even test whether two clips are equal.
//!
//! Wire format for encrypted values: `mem1:<base64(nonce || ciphertext)>`.
//! The version prefix lets a future scheme coexist with old data.

use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

const VALUE_PREFIX: &str = "mem1:";
const NONCE_LEN: usize = 24;
pub const KEY_LEN: usize = 32;

/// Domain separation for the HMAC key derived from the DEK.
const HASH_KEY_CONTEXT: &[u8] = b"memorafy.content_hash.v1";

pub type Key = [u8; KEY_LEN];

#[derive(Debug)]
pub enum CryptoError {
    /// Wrong key, tampered data, or malformed input.
    DecryptFailed,
    Malformed,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::DecryptFailed => write!(f, "decryption failed (wrong key or corrupted data)"),
            CryptoError::Malformed => write!(f, "malformed encrypted value"),
        }
    }
}

impl std::error::Error for CryptoError {}

pub fn generate_key() -> Key {
    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Derives the KEK from the user's password. The salt is derived from the
/// Supabase user id — unique per user and stable across devices, so every
/// device derives the same KEK from the same password with no extra
/// coordination. The id is hashed to a fixed 32-byte salt first, so the
/// KDF never depends on the id's length or format.
pub fn derive_kek(password: &str, user_id: &str) -> Result<Key, String> {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(b"memorafy.kek_salt.v1");
    hasher.update(user_id.as_bytes());
    let salt = hasher.finalize();

    let mut kek = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut kek)
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(kek)
}

fn cipher(key: &Key) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(key.into())
}

fn encrypt_bytes(key: &Key, plaintext: &[u8]) -> String {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    // Encryption with a valid key/nonce cannot fail in this AEAD.
    let ciphertext = cipher(key)
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .expect("AEAD encryption is infallible with valid inputs");
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    format!("{VALUE_PREFIX}{}", B64.encode(out))
}

fn decrypt_bytes(key: &Key, value: &str) -> Result<Vec<u8>, CryptoError> {
    let encoded = value.strip_prefix(VALUE_PREFIX).ok_or(CryptoError::Malformed)?;
    let raw = B64.decode(encoded).map_err(|_| CryptoError::Malformed)?;
    if raw.len() <= NONCE_LEN {
        return Err(CryptoError::Malformed);
    }
    let (nonce, ciphertext) = raw.split_at(NONCE_LEN);
    cipher(key)
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|_| CryptoError::DecryptFailed)
}

/// Encrypts a UTF-8 string field.
pub fn encrypt_str(key: &Key, plaintext: &str) -> String {
    encrypt_bytes(key, plaintext.as_bytes())
}

/// Encrypts arbitrary bytes (e.g. PNG blobs for storage upload).
pub fn encrypt_blob(key: &Key, plaintext: &[u8]) -> String {
    encrypt_bytes(key, plaintext)
}

/// Decrypts bytes produced by [`encrypt_blob`].
pub fn decrypt_blob(key: &Key, value: &str) -> Result<Vec<u8>, CryptoError> {
    decrypt_bytes(key, value)
}

/// Decrypts a field produced by [`encrypt_str`].
pub fn decrypt_str(key: &Key, value: &str) -> Result<String, CryptoError> {
    let bytes = decrypt_bytes(key, value)?;
    String::from_utf8(bytes).map_err(|_| CryptoError::Malformed)
}

/// True when a value carries our encrypted-format prefix.
pub fn is_encrypted_value(value: &str) -> bool {
    value.starts_with(VALUE_PREFIX)
}

/// Wraps the DEK under the KEK for server-side storage.
pub fn wrap_dek(kek: &Key, dek: &Key) -> String {
    encrypt_bytes(kek, dek)
}

/// Unwraps a server-stored DEK. Fails if the KEK (i.e. the password it was
/// derived from) doesn't match the one used to wrap.
pub fn unwrap_dek(kek: &Key, wrapped: &str) -> Result<Key, CryptoError> {
    let bytes = decrypt_bytes(kek, wrapped)?;
    let arr: [u8; KEY_LEN] = bytes.try_into().map_err(|_| CryptoError::Malformed)?;
    Ok(arr)
}

/// Deterministic keyed hash for the cloud `content_hash` column: preserves
/// cross-device dedupe without letting the server compare clips to guesses.
pub fn keyed_content_hash(dek: &Key, plaintext_hash_input: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&hash_key(dek))
        .expect("HMAC accepts any key length");
    mac.update(plaintext_hash_input.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn hash_key(dek: &Key) -> [u8; KEY_LEN] {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(dek).expect("HMAC accepts any key length");
    mac.update(HASH_KEY_CONTEXT);
    let out = mac.finalize().into_bytes();
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&out);
    key
}

/// Serialize/parse the DEK for OS-keychain caching (scoped to a user so an
/// account switch on the same machine can't reuse the wrong key).
pub fn encode_cached_dek(user_id: &str, dek: &Key) -> String {
    format!("{user_id}:{}", B64.encode(dek))
}

pub fn decode_cached_dek(user_id: &str, cached: &str) -> Option<Key> {
    let (cached_user, b64) = cached.split_once(':')?;
    if cached_user != user_id {
        return None;
    }
    let bytes = B64.decode(b64).ok()?;
    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let value = encrypt_str(&key, "hello encrypted world 🔐");
        assert!(is_encrypted_value(&value));
        assert_eq!(decrypt_str(&key, &value).unwrap(), "hello encrypted world 🔐");
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let value = encrypt_str(&generate_key(), "secret");
        assert!(matches!(
            decrypt_str(&generate_key(), &value),
            Err(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let key = generate_key();
        let mut value = encrypt_str(&key, "secret");
        // Flip a character near the end of the base64 payload.
        let flipped = if value.ends_with('A') { 'B' } else { 'A' };
        value.pop();
        value.push(flipped);
        assert!(decrypt_str(&key, &value).is_err());
    }

    #[test]
    fn dek_wrap_unwrap_roundtrip_and_wrong_password() {
        let dek = generate_key();
        let kek = derive_kek("correct horse battery staple", "user-123").unwrap();
        let wrapped = wrap_dek(&kek, &dek);

        assert_eq!(unwrap_dek(&kek, &wrapped).unwrap(), dek);

        let wrong_kek = derive_kek("wrong password", "user-123").unwrap();
        assert!(unwrap_dek(&wrong_kek, &wrapped).is_err());
    }

    #[test]
    fn same_password_same_user_derives_identical_kek() {
        let a = derive_kek("pw", "user-1").unwrap();
        let b = derive_kek("pw", "user-1").unwrap();
        let c = derive_kek("pw", "user-2").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn keyed_hash_is_deterministic_per_key() {
        let dek = generate_key();
        assert_eq!(keyed_content_hash(&dek, "x"), keyed_content_hash(&dek, "x"));
        assert_ne!(keyed_content_hash(&dek, "x"), keyed_content_hash(&dek, "y"));
        assert_ne!(
            keyed_content_hash(&dek, "x"),
            keyed_content_hash(&generate_key(), "x")
        );
    }

    #[test]
    fn cached_dek_is_scoped_to_user() {
        let dek = generate_key();
        let cached = encode_cached_dek("user-1", &dek);
        assert_eq!(decode_cached_dek("user-1", &cached), Some(dek));
        assert_eq!(decode_cached_dek("user-2", &cached), None);
    }
}
