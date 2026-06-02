param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$ReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable"
$AppRoot = Join-Path $ReleaseRoot "app"
$Exe = Join-Path $Root "src-tauri\target\release\voice-ime.exe"
$PreserveRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-preserve-" + [guid]::NewGuid().ToString("N"))
$PreservedModels = Join-Path $PreserveRoot "models"
$ModelCache = "D:\voice-ime-build-release\voice-ime-2.0.1-model-cache\models"

function Copy-DirectoryContents {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    if (-not (Test-Path -LiteralPath $Source)) {
        return
    }
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse -Force
}

function Stop-PortableRuntimeProcesses {
    $escapedReleaseRoot = [WildcardPattern]::Escape($ReleaseRoot)
    Get-CimInstance Win32_Process |
        Where-Object {
            $_.Name -in @("VoiceIME.exe", "llama-server.exe") -and (
                ($_.ExecutablePath -and $_.ExecutablePath -like "$escapedReleaseRoot*") -or
                ($_.CommandLine -and $_.CommandLine -like "*$ReleaseRoot*")
            )
        } |
        ForEach-Object {
            Write-Host "Stopping portable runtime process: $($_.Name) pid=$($_.ProcessId)"
            Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
        }
    Start-Sleep -Milliseconds 300
}

if (-not $SkipBuild) {
    Push-Location $Root
    try {
        Write-Host "Running Tauri production build. Do not use cargo build here; it can launch the dev URL."
        & npm.cmd run tauri build
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri production build failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "Production exe not found. Run from the project root with: npm run tauri build"
}

$frontend = Join-Path $Root "dist\index.html"
if (-not (Test-Path -LiteralPath $frontend -PathType Leaf)) {
    throw "Frontend dist is missing. Run: npm run build"
}

try {
    Stop-PortableRuntimeProcesses

    if (Test-Path -LiteralPath (Join-Path $AppRoot "models")) {
        Copy-DirectoryContents -Source (Join-Path $AppRoot "models") -Destination $PreservedModels
        Copy-DirectoryContents -Source (Join-Path $AppRoot "models") -Destination $ModelCache
    }

    if (Test-Path -LiteralPath $ReleaseRoot) {
        Remove-Item -LiteralPath $ReleaseRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $AppRoot | Out-Null

Copy-Item -LiteralPath $Exe -Destination (Join-Path $AppRoot "VoiceIME.exe")
Copy-Item -LiteralPath (Join-Path $Root "README.md") -Destination $AppRoot
Copy-Item -LiteralPath (Join-Path $Root "docs\acceptance.md") -Destination $AppRoot
Copy-Item -LiteralPath (Join-Path $Root "docs\2.0.1-roadmap.md") -Destination $AppRoot
if (Test-Path -LiteralPath (Join-Path $Root "CHANGELOG.md")) {
    Copy-Item -LiteralPath (Join-Path $Root "CHANGELOG.md") -Destination $AppRoot
}
$launcherText = @'
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
'@
Set-Content -LiteralPath (Join-Path $ReleaseRoot ([string][char]21551 + [string][char]21160 + [string][char]35821 + [string][char]38899 + [string][char]36755 + [string][char]20837 + ".bat")) -Value $launcherText -Encoding Default

foreach ($optional in @("models", "llama.cpp", "模型放置说明.md")) {
    $source = Join-Path "D:\voice-ime-build-release\voice-ime-1.1.5-portable" $optional
    if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination $AppRoot -Recurse
    }
}

if (Test-Path -LiteralPath $ModelCache) {
    $targetModels = Join-Path $AppRoot "models"
    Copy-DirectoryContents -Source $ModelCache -Destination $targetModels
}

if (Test-Path -LiteralPath $PreservedModels) {
    $targetModels = Join-Path $AppRoot "models"
    Copy-DirectoryContents -Source $PreservedModels -Destination $targetModels
}

$toolsDir = Join-Path $AppRoot "tools"
$miniCpmScript = Join-Path "D:\voice-ime-build-release\voice-ime-1.1.5-portable" "Start-MiniCPM-Translate.ps1"
if (Test-Path -LiteralPath $miniCpmScript) {
    New-Item -ItemType Directory -Path $toolsDir -Force | Out-Null
    $targetMiniCpmScript = Join-Path $toolsDir "Start-MiniCPM-Translate.ps1"
    Copy-Item -LiteralPath $miniCpmScript -Destination $targetMiniCpmScript
    $scriptBody = Get-Content -LiteralPath $targetMiniCpmScript -Raw
    $scriptBody = $scriptBody.Replace('$Root = $PSScriptRoot', '$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path')
    $scriptBody = $scriptBody.Replace("-WindowStyle Minimized", "-WindowStyle Hidden")
    Set-Content -LiteralPath $targetMiniCpmScript -Value $scriptBody -Encoding UTF8
}

foreach ($runtimeData in @(
    (Join-Path $ReleaseRoot ".voice_ime"),
    (Join-Path $AppRoot ".voice_ime")
)) {
    if (Test-Path -LiteralPath $runtimeData) {
        Remove-Item -LiteralPath $runtimeData -Recurse -Force
    }
}

$appItem = Get-Item -LiteralPath $AppRoot
$appItem.Attributes = $appItem.Attributes -bor [System.IO.FileAttributes]::Hidden

Write-Host "Portable release created: $ReleaseRoot"
}
finally {
    if (Test-Path -LiteralPath $PreserveRoot) {
        Remove-Item -LiteralPath $PreserveRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
