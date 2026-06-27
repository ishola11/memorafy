# Memora

**Your personal cross-device memory.**

Cross-platform desktop workspace for clipboard history, snippets, and cloud sync — built with Tauri 2, Rust, React, and SQLite.

## Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) (stable) — after install run `rustup default stable`
- **Windows:** [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the **Desktop development with C++** workload (provides `link.exe` for Rust)
- **Windows:** [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (usually preinstalled on Windows 11)

### Windows setup (one-time)

```powershell
# Rust
winget install Rustlang.Rustup
rustup default stable

# MSVC linker (required for Tauri on Windows)
winget install Microsoft.VisualStudio.2022.BuildTools
# Select: Desktop development with C++ workload
```

## Quick Start

```bash
cd memora
npm install
npm run tauri dev
```

**Port 1420 already in use?** A previous dev server may still be running. Use `dev.cmd` (it frees the port automatically), or run:

```powershell
Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

**PowerShell script blocked?** Use either:

```powershell
npm.cmd run tauri dev
```

Or double-click / run:

```text
dev.cmd
```

To fix PowerShell permanently (current user only):

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

## Keyboard Shortcuts

| Action | macOS | Windows |
|--------|-------|---------|
| Quick Paste launcher | `⌘⇧V` | `Ctrl+Shift+V` |
| Paste selected item | `Enter` | `Enter` |
| Switch History/Snippets | `Tab` | `Tab` |
| Close launcher | `Esc` | `Esc` |

## Project Structure

```
memora/
├── apps/desktop/          # Tauri + React app
│   ├── src/               # React UI (Quick Paste, Tray)
│   └── src-tauri/         # Rust core (clipboard, sync, SQLite)
├── packages/shared-types/ # Shared TypeScript types
├── services/migrations/   # Supabase PostgreSQL schema
└── docs/architecture/     # Architecture docs
```

## Development Status

See [PROGRESS.md](./PROGRESS.md) for implementation checklist.

## MVP Features

- [x] Text, URL, code, and image capture
- [x] SQLite + FTS5 instant search
- [x] Timeline sections (Now, Today, Yesterday…)
- [x] Smart preview cards
- [x] Quick Paste launcher (`Ctrl+Shift+V`)
- [x] System tray
- [x] Pin / Favorite / Rename / Delete
- [x] Collections (local)
- [x] Device management (local)
- [x] Offline sync queue (stub — Supabase Phase 2)
- [x] Supabase cloud sync
- [x] Snippets CRUD UI
- [x] Auto-update + signed installers (GitHub Releases + Tauri updater)

## Releases

1. **One-time:** `npm run generate:updater-keys` — creates minisign keypair; commit `apps/desktop/src-tauri/keys/memora.key.pub`
2. **GitHub secret:** paste `memora.key` contents into `TAURI_SIGNING_PRIVATE_KEY`
3. **Optional macOS code signing:** `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`
4. **Ship:** `git tag v0.1.0 && git push origin v0.1.0` — builds Windows + macOS installers and `latest.json` for in-app updates
5. **In-app:** Settings → About → Check for updates

## License

Private — all rights reserved.
