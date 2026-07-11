@echo off
REM Desktop dev launcher (uses @tauri-apps/cli from node_modules)
cd /d "%~dp0"
if not exist "node_modules\@tauri-apps\cli" (
  echo Installing npm dependencies...
  call npm install
  if errorlevel 1 exit /b 1
)
call npm run desktop
