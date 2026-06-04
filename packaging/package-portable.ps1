param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$ReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable"
$CoreReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core"
$AppRoot = Join-Path $ReleaseRoot "app"
$Exe = Join-Path $Root "src-tauri\target\release\voice-ime.exe"
$PreserveRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-preserve-" + [guid]::NewGuid().ToString("N"))
$PreservedModels = Join-Path $PreserveRoot "models"
$ModelCache = "D:\voice-ime-build-release\voice-ime-2.0.1-model-cache\models"
$ModelManifestSource = Join-Path $Root "packaging\model-manifest.json"

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

function Install-ModelManifest {
    param(
        [Parameter(Mandatory = $true)][string]$ModelsDir
    )
    if (-not (Test-Path -LiteralPath $ModelManifestSource -PathType Leaf)) {
        throw "Model manifest missing: $ModelManifestSource"
    }
    New-Item -ItemType Directory -Path $ModelsDir -Force | Out-Null
    Copy-Item -LiteralPath $ModelManifestSource -Destination (Join-Path $ModelsDir "MODELS.json") -Force

    $manifest = Get-Content -LiteralPath $ModelManifestSource -Raw | ConvertFrom-Json
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("# Voice IME Model Packs")
    $lines.Add("")
    $lines.Add("The app body and model packs are separated. Core portable packages do not include large model binaries; extract model packs into the listed target_dir to enable each profile.")
    $lines.Add("")
    $lines.Add("| profile | kind | status | target_dir | size |")
    $lines.Add("| --- | --- | --- | --- | --- |")
    foreach ($pack in $manifest.packs) {
        $lines.Add("| $($pack.profile) | $($pack.kind) | $($pack.status) | ``$($pack.target_dir)`` | ~$($pack.estimated_size_mb) MB |")
    }
    $lines.Add("")
    foreach ($pack in $manifest.packs) {
        $lines.Add("## $($pack.id)")
        $lines.Add("")
        $lines.Add("- Target dir: ``$($pack.target_dir)``")
        $lines.Add("- Required files:")
        foreach ($file in $pack.required_files) {
            $lines.Add("  - ``$file``")
        }
        if ($pack.source.mirror) {
            $lines.Add("- Mirror: $($pack.source.mirror)")
        }
        if ($pack.source.official) {
            $lines.Add("- Official: $($pack.source.official)")
        }
        $lines.Add("")
    }
    Set-Content -LiteralPath (Join-Path $ModelsDir "MODELS.md") -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
}

function Copy-PortableBody {
    param(
        [Parameter(Mandatory = $true)][string]$DestinationRoot,
        [Parameter(Mandatory = $true)][string]$LauncherText
    )
    $destinationApp = Join-Path $DestinationRoot "app"
    if (Test-Path -LiteralPath $DestinationRoot) {
        Remove-Item -LiteralPath $DestinationRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $destinationApp -Force | Out-Null

    foreach ($item in @(
        "VoiceIME.exe",
        "README.md",
        "CHANGELOG.md",
        "acceptance.md",
        "2.0.1-roadmap.md",
        "model-pack-strategy.md",
        "hotwords.md"
    )) {
        $source = Join-Path $AppRoot $item
        if (Test-Path -LiteralPath $source) {
            Copy-Item -LiteralPath $source -Destination $destinationApp -Force
        }
    }

    $sourceTools = Join-Path $AppRoot "tools"
    if (Test-Path -LiteralPath $sourceTools) {
        Copy-Item -LiteralPath $sourceTools -Destination $destinationApp -Recurse -Force
    }

    Install-ModelManifest -ModelsDir (Join-Path $destinationApp "models")
    Set-Content -LiteralPath (Join-Path $DestinationRoot ([string][char]21551 + [string][char]21160 + [string][char]35821 + [string][char]38899 + [string][char]36755 + [string][char]20837 + ".bat")) -Value $LauncherText -Encoding Default

    $runtimeData = Join-Path $destinationApp ".voice_ime"
    if (Test-Path -LiteralPath $runtimeData) {
        Remove-Item -LiteralPath $runtimeData -Recurse -Force
    }
    $appItem = Get-Item -LiteralPath $destinationApp
    $appItem.Attributes = $appItem.Attributes -bor [System.IO.FileAttributes]::Hidden
}

function Stop-PortableRuntimeProcesses {
    $releaseRoots = @($ReleaseRoot, $CoreReleaseRoot)
    Get-CimInstance Win32_Process |
        Where-Object {
            if ($_.Name -notin @("VoiceIME.exe", "llama-server.exe")) {
                return $false
            }
            foreach ($root in $releaseRoots) {
                $escapedRoot = [WildcardPattern]::Escape($root)
                if (($_.ExecutablePath -and $_.ExecutablePath -like "$escapedRoot*") -or
                    ($_.CommandLine -and $_.CommandLine -like "*$root*")) {
                    return $true
                }
            }
            return $false
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
Copy-Item -LiteralPath (Join-Path $Root "docs\model-pack-strategy.md") -Destination $AppRoot
Copy-Item -LiteralPath (Join-Path $Root "docs\hotwords.md") -Destination $AppRoot
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

Install-ModelManifest -ModelsDir (Join-Path $AppRoot "models")

$toolsDir = Join-Path $AppRoot "tools"
$miniCpmScript = Join-Path "D:\voice-ime-build-release\voice-ime-1.1.5-portable" "Start-MiniCPM-Translate.ps1"
if (Test-Path -LiteralPath $miniCpmScript) {
    New-Item -ItemType Directory -Path $toolsDir -Force | Out-Null
    $targetMiniCpmScript = Join-Path $toolsDir "Start-MiniCPM-Translate.ps1"
    Copy-Item -LiteralPath $miniCpmScript -Destination $targetMiniCpmScript
    $scriptBody = Get-Content -LiteralPath $targetMiniCpmScript -Raw
    $scriptBody = $scriptBody.Replace('$Root = $PSScriptRoot', '$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path')
    $scriptBody = $scriptBody.Replace("-WindowStyle Minimized", "-WindowStyle Hidden")
    $scriptBody = $scriptBody.Replace('NoteProperty translation_timeout 30', 'NoteProperty translation_timeout 8')
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
Copy-PortableBody -DestinationRoot $CoreReleaseRoot -LauncherText $launcherText
Write-Host "Core portable release created: $CoreReleaseRoot"
}
finally {
    if (Test-Path -LiteralPath $PreserveRoot) {
        Remove-Item -LiteralPath $PreserveRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
