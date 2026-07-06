# Copy this file's contents into GitHub → Settings → Secrets → TAURI_SIGNING_PRIVATE_KEY
# The secret is the single base64 line from memorafy.key (not the .pub file).
Get-Content "apps\desktop\src-tauri\keys\memorafy.key" -Raw | Set-Clipboard
Write-Host "Private key copied to clipboard. Paste into GitHub secret TAURI_SIGNING_PRIVATE_KEY."
