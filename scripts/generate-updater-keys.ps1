# Generate minisign keypair for Tauri updater artifacts.
# Private key stays local / in GitHub Actions secrets. Public key is committed.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Desktop = Join-Path $Root "apps\desktop"
$KeysDir = Join-Path $Desktop "src-tauri\keys"

New-Item -ItemType Directory -Force -Path $KeysDir | Out-Null

Push-Location $Desktop
try {
  $env:CI = "true"
  npx --yes @tauri-apps/cli signer generate -w "src-tauri/keys/memora.key" -f -p '""'
} finally {
  Pop-Location
}

Push-Location $Root
try {
  node "scripts/sync-updater-pubkey.mjs"
  Write-Host "Done. Commit src-tauri/keys/memora.key.pub and add memora.key content to GitHub secret TAURI_SIGNING_PRIVATE_KEY."
} finally {
  Pop-Location
}
