#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/apps/desktop"
KEYS="$DESKTOP/src-tauri/keys"

mkdir -p "$KEYS"
cd "$DESKTOP"
CI=true npx --yes @tauri-apps/cli signer generate -w "src-tauri/keys/memora.key" -f -p '""'
cd "$ROOT"
node scripts/sync-updater-pubkey.mjs
echo "Done. Commit src-tauri/keys/memora.key.pub and add memora.key to GitHub secret TAURI_SIGNING_PRIVATE_KEY."
