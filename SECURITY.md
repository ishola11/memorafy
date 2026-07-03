# Security Policy

Memora is a clipboard manager, which means it handles some of the most
sensitive data on a computer. Security reports are taken seriously and
handled with priority.

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

- Preferred: [GitHub private vulnerability reporting](https://github.com/ishola11/memora/security/advisories/new)
- Alternatively: email **work@mjaycloud.com** with a description and
  reproduction steps.

You'll get an acknowledgement within a few days. Please allow time for a fix
and coordinated release before public disclosure.

## Supported versions

Only the **latest release** receives security fixes. The built-in
auto-updater keeps installs current.

## Security model

What Memora does to protect clipboard data:

- **Local-first storage.** History lives in a SQLite database in your OS
  user profile; nothing leaves the device unless you sign in to sync.
- **End-to-end encrypted sync.** Synced clip content (text, titles,
  previews, URLs, and image bytes) is encrypted client-side with
  XChaCha20-Poly1305 under a random per-account data key. That key is
  wrapped with an Argon2id key derived from your password; the server only
  stores the wrapped blob. Content hashes sent to the server are HMACs, so
  the server cannot test clips for equality either.
- **Key storage.** Release builds keep the session and cached encryption
  key in the OS credential store (Windows Credential Manager / macOS
  Keychain).
- **Concealed-content exclusion.** Clips marked confidential by their
  source app (the convention password managers use) are never captured.
  Currently implemented on Windows; macOS is pending.
- **No plaintext downgrade.** If the encryption key is unavailable, sync
  pauses instead of uploading unencrypted content.
- **Server-side isolation.** Supabase row-level security scopes every row
  to its owner; device registration and item upserts go through
  `SECURITY DEFINER` RPCs with ownership checks.

## Known limitations (disclosed by design)

- Sync **metadata** is not encrypted: timestamps, item kind/content-type,
  pinned/favorite flags, character counts, device names, and collection
  names are visible to the server operator.
- A **password reset** (as opposed to a signed-in password change) discards
  access to the old key: previously synced clips become unrecoverable unless
  another signed-in device still holds the key and heals the account.
- The local SQLite database is **not encrypted at rest**. It relies on OS
  user-account isolation and full-disk encryption (BitLocker/FileVault).
- Anyone with access to your unlocked OS session can read your clipboard
  history, the same trust boundary as the OS clipboard itself.
- Debug/dev builds store secrets in the local database rather than the OS
  keychain (documented in `keychain.rs`); release builds are unaffected.
