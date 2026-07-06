@echo off
setlocal
cd /d "%~dp0"

REM Stop leftover Memorafy instances (tray apps often survive dev restarts)
taskkill /F /IM memorafy-desktop.exe >nul 2>&1

REM Free Vite dev port if a previous run is still listening
for /f "tokens=5" %%a in ('netstat -ano ^| findstr ":1420" ^| findstr "LISTENING"') do (
  taskkill /F /PID %%a >nul 2>&1
)

npm.cmd run tauri dev
