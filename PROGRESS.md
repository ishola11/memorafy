# Memorafy — Implementation Progress

## Phase 0: Foundation ✅

- [x] Monorepo, Tauri 2, React UI, SQLite + FTS5
- [x] Clipboard watcher, tray, Quick Paste (`Ctrl+Shift+V`)
- [x] Timeline, search, preview cards, pin/favorite

## Phase 1: Local App Verified ✅

- [x] App runs on Windows, single tray icon, Vite EBUSY fix

## Phase 2: Cloud Sync ✅

- [x] **Step 14** — Supabase REST push/pull (items + devices)
- [x] **Step 14b** — Realtime WebSocket listener (postgres_changes)
- [x] **Step 14c** — Auth (email/password), session persistence
- [x] **Step 16** — Device transfer toast ("Available on…")
- [x] **Step 17** — Settings window (sign in, devices, sync status)
- [x] **Step 17b** — History retention (30/60/90 days, keeps pins/favorites/collections)
- [x] **Collections cloud sync** — push/pull collections + item_collections, realtime
- [x] **Step 15** — Snippets library UI + CRUD
- [x] **Step 18** — Signed installers + auto-update

## Phase 2.5: Sync & UX Polish ✅

- [x] **A** — Fix sync echo loop (programmatic hash + content_hash dedupe in watcher)
- [x] **B** — Tab navigation in Tray + Quick Paste (History | Pinned | Favorites | Collections | Snippets)
- [x] **C** — Wire Collections tab (chips + footer filter by collection_id)
- [x] **D** — Pause clipboard watching (setting + tray toggle + IPC)
- [x] **E** — Snippets tab groundwork (filter `kind='snippet'`)
- [x] Scrollable settings panel fix

## Phase 3: UI/UX Overhaul ✅

- [x] **Collections CRUD** — Settings UI + cloud sync
- [x] **Assign to collections** — PreviewCard folder menu (tray + quick paste)
- [x] **Professional UI** — TabBar, design tokens, action bar on PreviewCard
- [x] **Tray popover** — Position at tray click rect (Retina-aware), no center-on-show
- [x] **Quick Paste** — Centers on cursor monitor, not primary desktop
- [x] **Theme system** — System / Light / Dark
- [x] **Enhanced dedupe** — 5 min hash window, plain-text dedupe, longer suppress

## Phase 3.1: Sync & Mac polish (2026-06-27)

- [x] **Collections FK sync** — push/pull order: collections → items → item_collections; defer link push until parents synced; `ensure_collection_exists` on pull
- [x] **Mac menubar overlay** — `visibleOnAllWorkspaces`, `acceptFirstMouse`, NSWindow level + collection behavior for tray + quick-paste
- [x] **PreviewCard actions** — header row layout; icons top-right (no text overlay); collection dropdown below button

## Phase 3.2: Mac menubar + action feedback (2026-06-27)

- [x] **Mac menubar popover** — `ActivationPolicy::Accessory` + `LSUIElement`, `NSPopUpMenuWindowLevel`, non-activating panel style, `orderFrontRegardless`, OR'd `collectionBehavior` flags (`macos_popover.rs`)
- [x] **Action toasts** — `ActionToast` bottom-left, 2s auto-dismiss; wired for copy/pin/favorite/collection/delete in Tray + Quick Paste
- [x] **PreviewCard UX** — loading spinners per action, collection checkmark flash, menu closes after toggle

## Phase 3.3: Full-screen Space overlay (2026-06-27)

- [x] **Option A** — Tauri webview + AppKit flags (not native NSPopover rewrite)
- [x] Remove `Stationary`; add `MoveToActiveSpace` + `canAppearWhileOtherAppIsFullScreen`
- [x] `ensure_accessory_policy` before every popover show; restore after Settings closes
- [x] No `set_focus` on popover show; `orderFrontRegardless` only
- [x] NSPanel selectors guarded with `respondsToSelector:`
- [x] **Stability fix** — removed native `msg_send!` AppKit config (NSExceptions abort process); Tauri-only popover show until native NSPopover phase
- [x] **macOS tray UX** — native menubar menu on left-click; Quick Paste as primary UI (no custom tray webview panel)

## Phase 3.4: Native NSPopover menubar (2026-06-27)

- [x] **Native NSPopover** — `tauri-plugin-nspopover` v4.1.0; tray webview → full TrayPanel in popover
- [x] **Mac left-click** — toggle NSPopover (not native menu-only); right-click keeps tray menu
- [x] **Quick Paste** — separate overlay window (`⌘⇧V`); hides tray popover when opened
- [x] **Settings** — `ActivationPolicy::Regular` on open, Accessory restore on close; hides tray popover
- [x] **Windows** — unchanged custom sidebar panel on left-click

## Phase 4: Production Hardening — Wave 1 (2026-07-02)

- [x] **Logging** — daily-rotating file logs (7 kept) in the app data dir (`logs/`), panic hook so release aborts leave a trace, "Open logs folder" in Settings → About
- [x] **Single instance** — second launch focuses the running app instead of racing on SQLite
- [x] **Capture correctness** — removed dev-specific noise filter that silently dropped user text; image dedupe hashes pixels (not just dimensions); watcher retries clipboard init; >10 MB text skipped with a log line
- [x] **Auth resilience** — transient refresh failures (5xx/429/network) no longer sign the user out; friendly sign-in error messages; malformed keys fail gracefully instead of panicking
- [x] **Sync engine** — event-driven wakeups (2 s only while pending, 30 s idle fallback); per-entity exponential push backoff (5 s → 10 min); incremental pull every 60 s via `updated_at` cursor (recovers missed realtime events, includes deletions); realtime JWT renewed on-socket before expiry; catch-up pull on every realtime (re)connect; synced queue rows pruned hourly
- [x] **Data integrity** — multi-statement writes (items + FTS index + sync queue) are transactional; remote upserts respect newer local pending edits (last-write-wins by `updated_at`); `items(sync_status)` index
- [x] **Feedback** — new Settings section: bug reports & feature requests with diagnostics preview and explicit consent; provider-abstracted submission (currently drafts a prefilled GitHub issue the user reviews in their browser)
- [x] **Cleanup** — removed fake "Copy as plain text" action, dead `clear_all_history`/`parse_expires_at`, unreferenced local migration; card action failures now show an error toast; versions unified at 0.1.8

## Phase 4: Production Hardening — Wave 2 (2026-07-03)

- [x] **Keychain token storage** — auth session (access/refresh tokens) moved from plaintext SQLite to the OS credential store (Windows Credential Manager / macOS Keychain via `keyring`); existing plaintext sessions migrate automatically on next load; falls back to local storage (with a logged warning) if no OS backend is available rather than breaking sign-in
- [x] **Concealed-clipboard exclusion (Windows)** — detects the `ExcludeClipboardContentFromMonitorProcessing` clipboard format that password managers (1Password, Bitwarden, KeePass) and Windows' own Clipboard History use to mark secrets, and skips capture entirely; verified live against the real clipboard. **macOS equivalent (`org.nspasteboard.ConcealedType`) is not yet implemented** — requires native AppKit bindings that couldn't be compiled/verified outside a macOS environment; tracked as follow-up, logs a one-time warning in the meantime
- [x] **Content Security Policy** — `tauri.conf.json` had `csp: null`; now a strict policy (`script-src 'self'`, no `unsafe-eval`) scoped to what the app actually loads (Google Fonts, Tauri IPC, local assets). Verified against the running dev build: Settings, tray search, and sync status all still work
- [x] **Stopped syncing local blob_path** — image items were pushing their local absolute filesystem path (leaking the username/directory layout) to Supabase, where it was never valid on another device anyway; now always `NULL` on push and pull. `services/migrations/007_clear_leaked_blob_paths.sql` scrubs already-synced data
- [x] **Fixed a live startup crash found during verification** — global shortcut registration failure (e.g. another app already holds `Ctrl+Shift+V`) was aborting the whole app via `?` in the setup hook; now logs a warning and continues (Quick Paste stays reachable from the tray menu)

## Phase 4: Production Hardening — Wave 3 (2026-07-03)

- [x] **Account lifecycle** — sign-up (handles both email-confirmation-on and -off Supabase projects, with resend), forgot/reset password (enumeration-safe wording), change password (Settings → Account, signed-in view), show-password toggles, friendly error mapping throughout
- [x] **First-launch onboarding** — 3-step welcome hosted in the settings window (auto-opened on first launch since the app is tray-only): features + Quick Paste shortcut, privacy explainer (local-first, concealed-clipboard exclusion, honest not-yet-E2E disclosure), launch-at-login toggle (on by default), sync sign-in/sign-up/skip step that adapts when the build has no Supabase config. Verified end-to-end against the running app
- [x] **Honest sync badge** — tray badge now reflects actual sync health (green Synced / amber "N pending" / gray "Local only") instead of just login state
- [x] **Empty states** — history/pinned/favorites tabs teach the feature instead of "Nothing here yet"
- [x] **Startup bootstrap fix (found during verification)** — bootstrap ran with the stored (possibly expired) JWT, so device registration failed with "JWT expired" after the app had been closed >1 hour; the session is now refreshed before bootstrap

## Phase 4: Pre-Wave-4 fixes (2026-07-03)

- [x] **Duplicate devices fixed** — repair_sync no longer rotates the device id on every run (the main duplicate generator); `008_dedupe_devices.sql` makes the register_device RPC self-healing (stale same-name rows are merged into the registering device, items reattributed, with a 10-minute activity guard so two live machines sharing a hostname can't delete each other); local stale device rows pruned during bootstrap; the Devices settings page now refreshes from the cloud on open
- [x] **Tray "Sync now"** — new tray-menu action runs a full pull+push in the background and reports the outcome as a toast in open panels
- [x] **Icon redesign** — new stacked-clip-cards mark (mirrors the product UI) with gradient background; full icon set regenerated via `tauri icon`; matching monochrome macOS menubar template; verified legible at Windows tray size

## Phase 4: End-to-End Encryption (2026-07-03)

- [x] **E2E scheme** — item content (plain_text, titles, previews, URLs, triggers) is encrypted client-side with XChaCha20-Poly1305 under a random per-account data key (DEK); the DEK is wrapped with an Argon2id key derived from the user's password and only the wrapped blob is stored server-side (`user_encryption_keys` migration). The server can no longer read clipboard content
- [x] **Private dedupe** — cloud `content_hash` is now an HMAC under a DEK-derived key, so the server can't test clips for equality against guesses; local dedupe still uses plain SHA-256 (recomputed on decrypt)
- [x] **Key lifecycle** — generated at sign-up, unwrapped at sign-in, cached in the OS keychain per device; password *change* re-wraps (no data loss); password *reset* auto-heals if any signed-in device still holds the key, otherwise Settings → Account shows an Unlock card (re-enter password) with a clearly destructive "Reset sync encryption" fallback
- [x] **No plaintext downgrade** — while the key is locked, item pushes pause (deletions still sync) and the tray badge shows "Sync locked"; nothing is ever uploaded unencrypted once E2E is active
- [x] **Backfill** — one-time re-push re-encrypts previously synced plaintext rows (including soft-deleted ones)
- [x] Verified live: locked-state detection on startup ("no cached encryption key — sync decryption locked until next sign-in") + 7 new crypto unit tests (roundtrip, tamper rejection, wrong-password unwrap failure, KDF determinism)
- Known limitations: collection *names* and structural metadata (timestamps, pinned flags, char counts) are not encrypted; macOS concealed-clipboard detection still pending

## Phase 5: Open-Source Release Package — Wave 4 (2026-07-03)

- [x] **MIT license** — LICENSE file + license metadata in all manifests
- [x] **Public README** — full rewrite for end users and contributors: features, install, E2E explanation, self-hosting walkthrough, dev setup, release process, FAQ/troubleshooting, fork notes (updater endpoint + signing keys)
- [x] **Community files** — CONTRIBUTING.md (incl. schema-change workflow and encryption rules for new synced fields), SECURITY.md (private reporting + honest threat model and limitations), CODE_OF_CONDUCT.md (Contributor Covenant 2.1), CHANGELOG.md (Keep-a-Changelog)
- [x] **PR CI** — .github/workflows/ci.yml runs tsc, production frontend build, and cargo test on windows-latest for every PR/push (release.yml remains tag-only)
- [x] **v0.2.0** — version unified across all four manifests for the first public release

## Next Up

1. Run `003_collections_realtime.sql` in Supabase if project predates this update
2. Test Mac ↔ Windows collection sync
3. Generate updater keys: `npm run generate:updater-keys` (once), commit `memorafy.key.pub`, add private key to GitHub secret `TAURI_SIGNING_PRIVATE_KEY`
4. Tag a release (`git tag v0.1.0 && git push origin v0.1.0`) to trigger signed builds via GitHub Actions

## Supabase Setup

**Canonical schema:** `services/supabase/migrations/` (CLI-managed, deployed via the
Supabase GitHub integration). The one-time dashboard setup:

1. Create project at [supabase.com](https://supabase.com)
2. Dashboard → Project Settings → Integrations → GitHub → connect this repository
3. Set **working directory** to `services/supabase` ([docs](https://supabase.com/docs/guides/deployment/branching/github-integration#set-the-working-directory))
4. Set the **production branch** to `master` and enable branching if you want PR preview databases
5. Copy project URL + anon key to `apps/desktop/.env`
6. **Authentication → URL Configuration:** set **Site URL** to `memorafy://auth/callback` and add the same URL under **Redirect URLs** (exact match). Email confirmation and password-reset links then reopen the desktop app instead of a browser tab.
7. **Authentication → Email Templates:** update **Confirm signup** and **Reset password** templates to show the 6-digit code (`{{ .Token }}`) instead of a magic link. See `services/supabase/templates/confirmation.html` and `recovery.html` for copy. Users enter the code in the Memorafy app (Settings → Account).

Merges to `master` then apply new files in `services/supabase/migrations/`
automatically; PRs get isolated preview databases.

**Manual fallback (no GitHub integration):** run
`services/supabase/migrations/20260703000000_baseline.sql` once in the SQL editor —
it's idempotent and contains the full current schema. The old step-by-step scripts in
`services/migrations/` are legacy (see the README there).
5. Create a test user (Authentication → Users → Add user)
6. Copy project URL + anon key to `apps/desktop/.env`

```env
SUPABASE_URL=https://xxxx.supabase.co
SUPABASE_ANON_KEY=eyJ...
```
