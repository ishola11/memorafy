# Changelog

## [0.1.9] - 2026-07-05

### Changed
- Rebrand from Memora to Memorafy (product name, npm packages, bundle ID, deep links, GitHub org). Version held at **0.1.9** for the memorafy.com launch rather than bumping semver for a rename-only release.
All notable changes to Memorafy are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased] - 0.2.0

The production-hardening release: Memorafy's first public, open-source version.

### Added
- **End-to-end encryption** for synced clips: content is encrypted on-device
  (XChaCha20-Poly1305) under a per-account key wrapped by an Argon2id
  password-derived key; content hashes are HMACs; images are encrypted too.
  Unlock/Reset flows in Settings for key recovery scenarios.
- **Accounts**: in-app sign-up with email OTP confirmation, forgot/reset
  password (OTP and `memorafy://` deep-link email flows), change password with
  automatic key re-wrap.
- **First-launch onboarding**: welcome, privacy explainer, and optional sync
  setup; launch-at-login prompt.
- **Image sync**: clipboard images upload (encrypted) to Supabase Storage and
  download on other devices, with thumbnails and copy-back to the clipboard.
- **In-app Feedback** (Settings → Feedback): bug reports and feature requests
  drafted as GitHub issues with an explicit, previewable diagnostics opt-in.
- **Tray**: "Sync now" menu action with result toast; honest sync badge
  (Synced / N pending / Sync locked / Local only).
- **Erase all local data** (Settings → General): factory-reset the app,
  including keychain entries.
- Production logging: daily-rotating files, panic hook, "Open logs folder".
- Single-instance guard; second launches focus the running app.
- Concealed-clipboard exclusion on Windows (password manager clips are never
  captured).
- New app icon and macOS menubar template.

### Changed
- Sync engine rebuilt for reliability: event-driven push with exponential
  backoff, incremental pull cursor (recovers anything missed offline),
  realtime socket JWT renewal, session refresh before bootstrap, and
  device-registration self-healing (no more duplicate devices).
- Auth tokens and the encryption key moved to the OS credential store
  (Windows Credential Manager / macOS Keychain) in release builds.
- Supabase schema is now CLI-managed (`services/supabase/migrations`) and
  deployable via the Supabase GitHub integration.
- Strict Content-Security-Policy; friendlier error messages throughout.
- "Copy as plain text" is now meaningfully distinct from Copy (URLs copy as
  clickable links with a plain-text fallback).

### Fixed
- Clipboard captures containing "error:"/"warning:" were silently dropped.
- Two same-size screenshots deduped as one item (image hashing now uses
  pixel data).
- Live sync silently stopped after ~1 hour (realtime token expiry).
- Transient server errors during token refresh signed the user out.
- Global-shortcut conflicts crashed the app at startup.
- Multi-statement database writes are now transactional (no more search
  index drift after a crash).
- Local filesystem paths are no longer pushed to the cloud.

## [0.1.8] and earlier

Internal pre-release iterations: local clipboard history, FTS5 search,
Quick Paste overlay, tray panel, pins/favorites/collections/snippets,
Supabase sync (items, collections, devices), realtime updates, retention,
signed installers with auto-update. See git history for details.
