# Memora — Implementation Progress

## Phase 0: Foundation ✅

- [x] Monorepo, Tauri 2, React UI, SQLite + FTS5
- [x] Clipboard watcher, tray, Quick Paste (`Ctrl+Shift+V`)
- [x] Timeline, search, preview cards, pin/favorite

## Phase 1: Local App Verified ✅

- [x] App runs on Windows, single tray icon, Vite EBUSY fix

## Phase 2: Cloud Sync (current)

- [x] **Step 14** — Supabase REST push/pull (items + devices)
- [x] **Step 14b** — Realtime WebSocket listener (postgres_changes)
- [x] **Step 14c** — Auth (email/password), session persistence
- [x] **Step 16** — Device transfer toast ("Available on…")
- [x] **Step 17** — Settings window (sign in, devices, sync status)
- [x] **Step 17b** — History retention (30/60/90 days, keeps pins/favorites/collections)
- [x] **Collections cloud sync** — push/pull collections + item_collections, realtime
- [ ] **Step 15** — Snippets library UI + CRUD
- [ ] **Step 18** — Signed installers + auto-update

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

## Next Up

1. Run `003_collections_realtime.sql` in Supabase if project predates this update
2. Test Mac ↔ Windows collection sync
3. Snippets library UI
4. Signed installers + auto-update

## Supabase Setup

1. Create project at [supabase.com](https://supabase.com)
2. Run `services/migrations/SETUP_ALL.sql` in SQL editor (new projects)
3. Existing projects: also run `services/migrations/003_collections_realtime.sql`
4. Run `services/migrations/002_plain_text_realtime.sql` if not already applied
5. Create a test user (Authentication → Users → Add user)
6. Copy project URL + anon key to `apps/desktop/.env`

```env
SUPABASE_URL=https://xxxx.supabase.co
SUPABASE_ANON_KEY=eyJ...
```
