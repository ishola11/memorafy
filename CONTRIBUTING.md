# Contributing to Memorafy

Thanks for helping make Memorafy better! This guide covers everything you need
to get from `git clone` to a merged PR.

## Getting set up

Prerequisites and run instructions are in the [README](README.md#development).
Short version:

```bash
npm install
npm run tauri dev
```

Cloud sync is optional during development. Without `apps/desktop/.env` the
app runs local-only, which is enough for most UI and clipboard work. For sync
work, create a free Supabase project and follow
[Self-hosting cloud sync](README.md#self-hosting-cloud-sync).

Dev builds intentionally store secrets in the local database instead of the
OS keychain (see `keychain.rs`) so unsigned macOS binaries don't trigger
keychain password prompts on every run.

## Project layout

| Path | What lives there |
|---|---|
| `apps/desktop/src/` | React UI: tray panel, Quick Paste, settings, onboarding |
| `apps/desktop/src-tauri/src/clipboard/` | Clipboard watcher, image capture, concealed-content detection |
| `apps/desktop/src-tauri/src/sync/` | Sync engine, Supabase client, realtime socket, auth |
| `apps/desktop/src-tauri/src/db/` | SQLite schema, queries, FTS5 search index |
| `apps/desktop/src-tauri/src/crypto.rs` | End-to-end encryption (read this before touching sync payloads) |
| `packages/shared-types/` | TypeScript types shared between UI and IPC |
| `services/supabase/` | Cloud schema (CLI-managed migrations) |

## Making changes

- **Small, focused PRs** merge fastest. One fix or feature per PR.
- **Match the surrounding style.** The Rust side favors explicit error
  handling: `Result<_, String>` at IPC boundaries, `tracing` for logs, no
  `println!`, and no `unwrap()` on fallible paths. The UI side is
  function components + Tailwind, with user-visible errors surfaced as
  toasts, never silently swallowed.
- **No silent failures.** Every failure path needs a log line and, where a
  user would otherwise be confused, UI feedback.
- **Encrypted fields:** anything content-bearing that syncs must go through
  `crypto.rs`. If you add a synced field containing user content, encrypt
  it in `item_to_cloud` and decrypt it in `decrypt_incoming`.

### Database / schema changes

- **Local (SQLite):** add idempotent statements to `apply_schema_patches` in
  `db/queries.rs` (the patch list runs on every startup).
- **Cloud (Supabase):** add a **new timestamped file** in
  `services/supabase/migrations/` (e.g.
  `20260801120000_add_thing.sql`) in the same PR as the code using it.
  Never edit an already-committed migration, and never apply schema changes
  through the dashboard SQL editor (it desyncs migration history).

### Tests

```bash
cd apps/desktop/src-tauri && cargo test
cd apps/desktop && npx tsc --noEmit && npm run build
```

Add tests alongside non-trivial logic, especially for crypto, database, and
sync changes. The existing patterns in `crypto.rs` and `db/queries.rs` (temp-dir
SQLite fixtures) are easy to copy. Note some clipboard tests only run on
Windows (they exercise the real Win32 clipboard).

## Commit & PR conventions

- Commit messages: conventional-ish prefixes (`feat:`, `fix:`, `chore:`,
  `docs:`) with a body explaining *why* when it isn't obvious.
- Describe user-visible behavior changes in the PR description.
- CI must pass (Rust tests + TypeScript + production build).

## Reporting bugs & requesting features

Use **Settings → Feedback** inside the app (it pre-fills a GitHub issue with
optional diagnostics you can review), or open an issue directly. Include
your OS, app version (Settings → About), and relevant lines from
**Settings → About → Open logs folder**.

**Security issues: do not open a public issue.** See [SECURITY.md](SECURITY.md).

## Code of conduct

Participation in this project is covered by our
[Code of Conduct](CODE_OF_CONDUCT.md). Be kind.
