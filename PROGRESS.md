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

- [x] **Collections CRUD** — `create_collection`, `update_collection`, `delete_collection` IPC + Settings UI (name, color presets, rename/delete)
- [x] **Professional UI** — Underline TabBar, sticky header + scrollable content, design tokens (`surface`, `border`, `accent`), TrayPanel + QuickPasteLauncher polish
- [x] **Tray popover behavior** — Position panel at tray click coords (`skipTaskbar`, `decorations: false`, `alwaysOnTop`); macOS below menubar, Windows above taskbar
- [x] **Theme system** — System / Light / Dark persisted in SQLite (`theme_preference`), class-based Tailwind dark mode, `theme-changed` event
- [x] **Settings sidebar** — Account & Sync, Devices, History, Collections, Appearance sections

## Next Up

1. Create Supabase project + run migrations
2. Copy `.env.example` → `apps/desktop/.env`
3. Sign in via Settings on both devices
4. Test Mac ↔ Windows sync
5. Wire `item_collections` assign UI (add clips to collections from tray)

## Supabase Setup

1. Create project at [supabase.com](https://supabase.com)
2. Run `services/migrations/001_cloud_schema.sql` in SQL editor
3. Run `services/migrations/002_plain_text_realtime.sql`
4. Enable Realtime: `ALTER PUBLICATION supabase_realtime ADD TABLE public.items;`
5. Create a test user (Authentication → Users → Add user)
6. Copy project URL + anon key to `apps/desktop/.env`

```env
SUPABASE_URL=https://xxxx.supabase.co
SUPABASE_ANON_KEY=eyJ...
```
