@echo off
setlocal
cd /d "%~dp0app"
set "VOICE_IME_ROOT=%~dp0app"
if not defined VOICE_IME_MODEL_DIR if exist "%~dp0app\MODEL_ROOT.txt" (
  for /f "usebackq eol=# tokens=* delims=" %%M in ("%~dp0app\MODEL_ROOT.txt") do if not defined VOICE_IME_MODEL_DIR set "VOICE_IME_MODEL_DIR=%%M"
)
if not exist "%~dp0app\VoiceIME.exe" (
  echo VoiceIME.exe not found.
  pause
  exit /b 1
)
start "" "%~dp0app\VoiceIME.exe"
