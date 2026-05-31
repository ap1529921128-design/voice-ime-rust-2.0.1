@echo off
setlocal
cd /d "%~dp0app"
set "VOICE_IME_ROOT=%~dp0app"
if not exist "%~dp0app\VoiceIME.exe" (
  echo VoiceIME.exe not found.
  pause
  exit /b 1
)
start "" "%~dp0app\VoiceIME.exe"
