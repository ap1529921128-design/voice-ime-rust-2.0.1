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
$LauncherName = ([string][char]21551 + [string][char]21160 + [string][char]35821 + [string][char]38899 + [string][char]36755 + [string][char]20837 + ".bat")
$DoctorLauncherName = $LauncherName.Replace(".bat", "-" + [string][char]35786 + [string][char]26029 + ".bat")

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

function Get-OptionalCommandOutput {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [string[]]$Arguments = @(),
        [switch]$AllowEmpty
    )
    try {
        $output = & $Command @Arguments 2>$null
        if ($LASTEXITCODE -ne 0) {
            return "unknown"
        }
        $value = ($output -join " ").Trim()
        if ([string]::IsNullOrWhiteSpace($value)) {
            if ($AllowEmpty) {
                return ""
            }
            return "unknown"
        }
        return $value
    }
    catch {
        return "unknown"
    }
}

function Write-BuildStamp {
    param(
        [Parameter(Mandatory = $true)][string]$DestinationApp,
        [Parameter(Mandatory = $true)][string]$PackageName
    )
    $tauriConfig = Get-Content -LiteralPath (Join-Path $Root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
    $packageJson = Get-Content -LiteralPath (Join-Path $Root "package.json") -Raw | ConvertFrom-Json

    Push-Location $Root
    try {
        $commit = Get-OptionalCommandOutput -Command "git" -Arguments @("rev-parse", "--short", "HEAD")
        $branch = Get-OptionalCommandOutput -Command "git" -Arguments @("rev-parse", "--abbrev-ref", "HEAD")
        $status = Get-OptionalCommandOutput -Command "git" -Arguments @("status", "--short") -AllowEmpty
        if ($status -eq "unknown") {
            $status = "unknown"
        }
        elseif ([string]::IsNullOrWhiteSpace($status)) {
            $status = "clean"
        }
    }
    finally {
        Pop-Location
    }

    $lines = @(
        "Voice IME Build",
        "package=$PackageName",
        "product=$($tauriConfig.productName)",
        "version=$($tauriConfig.version)",
        "npm_package_version=$($packageJson.version)",
        "built_at=$((Get-Date).ToString("o"))",
        "source_root=$Root",
        "git_branch=$branch",
        "git_commit=$commit",
        "git_status=$status",
        "rustc=$(Get-OptionalCommandOutput -Command "rustc" -Arguments @("--version"))",
        "node=$(Get-OptionalCommandOutput -Command "node" -Arguments @("--version"))",
        "tauri_cli=$(Get-OptionalCommandOutput -Command "npx.cmd" -Arguments @("tauri", "--version"))"
    )
    Set-Content -LiteralPath (Join-Path $DestinationApp "BUILD.txt") -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
}

function Install-ToolScripts {
    param(
        [Parameter(Mandatory = $true)][string]$DestinationApp
    )
    $toolsDir = Join-Path $DestinationApp "tools"
    New-Item -ItemType Directory -Path $toolsDir -Force | Out-Null
    $sourceToolsDir = Join-Path $Root "packaging\tools"
    if (Test-Path -LiteralPath $sourceToolsDir -PathType Container) {
        Copy-Item -Path (Join-Path $sourceToolsDir "*") -Destination $toolsDir -Recurse -Force
    }
    $doctorText = @'
@echo off
setlocal
cd /d "%~dp0.."
if not exist "VoiceIME.exe" (
  echo VoiceIME.exe not found.
  pause
  exit /b 1
)
start /wait "" "%CD%\VoiceIME.exe" --doctor
echo.
echo 诊断已完成，日志目录：
echo %CD%\.voice_ime\logs
if exist "%CD%\.voice_ime\logs" start "" "%CD%\.voice_ime\logs"
pause
'@
    Set-Content -LiteralPath (Join-Path $toolsDir $DoctorLauncherName) -Value $doctorText -Encoding Default
}

function Assert-PortableLayout {
    param(
        [Parameter(Mandatory = $true)][string]$DestinationRoot,
        [switch]$CorePackage
    )
    $launcher = Join-Path $DestinationRoot $LauncherName
    $destinationApp = Join-Path $DestinationRoot "app"
    if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
        throw "Portable gate failed: launcher missing at $launcher"
    }
    if (-not (Test-Path -LiteralPath $destinationApp -PathType Container)) {
        throw "Portable gate failed: hidden app directory missing at $destinationApp"
    }
    $appItem = Get-Item -LiteralPath $destinationApp -Force
    if (-not (($appItem.Attributes -band [System.IO.FileAttributes]::Hidden) -eq [System.IO.FileAttributes]::Hidden)) {
        throw "Portable gate failed: app directory is not hidden"
    }
    $rootItems = @(Get-ChildItem -LiteralPath $DestinationRoot -Force)
    $unexpectedRootItems = @($rootItems | Where-Object { $_.Name -notin @("app", $LauncherName) })
    if ($unexpectedRootItems.Count -gt 0) {
        throw "Portable gate failed: unexpected root items: $($unexpectedRootItems.Name -join ', ')"
    }
    $visibleRootItems = @(Get-ChildItem -LiteralPath $DestinationRoot)
    if ($visibleRootItems.Count -ne 1 -or $visibleRootItems[0].Name -ne $LauncherName) {
        throw "Portable gate failed: root must visibly expose only $LauncherName"
    }

    foreach ($required in @("VoiceIME.exe", "README.md", "BUILD.txt", "models\MODELS.json", "models\MODELS.md")) {
        $requiredPath = Join-Path $destinationApp $required
        if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
            throw "Portable gate failed: required app file missing: $required"
        }
    }

    $forbidden = @(Get-ChildItem -LiteralPath $DestinationRoot -Force -Recurse -Directory |
        Where-Object { $_.Name -in @(".voice_ime", "recordings", "backup", "backups") })
    if ($forbidden.Count -gt 0) {
        throw "Portable gate failed: forbidden runtime directories found: $($forbidden.FullName -join '; ')"
    }

    if ($CorePackage) {
        $modelsDir = Join-Path $destinationApp "models"
        $modelItems = @(Get-ChildItem -LiteralPath $modelsDir -Force)
        $unexpectedModels = @($modelItems | Where-Object { $_.Name -notin @("MODELS.json", "MODELS.md") })
        if ($unexpectedModels.Count -gt 0) {
            throw "Portable gate failed: core package models directory contains binaries or extra files: $($unexpectedModels.Name -join ', ')"
        }
    }
}

function Copy-PortableBody {
    param(
        [Parameter(Mandatory = $true)][string]$DestinationRoot,
        [Parameter(Mandatory = $true)][string]$LauncherText,
        [switch]$CorePackage
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
        "release-notes-2.0.1.md",
        "translation-benchmark.md",
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
    Write-BuildStamp -DestinationApp $destinationApp -PackageName $(if ($CorePackage) { "core" } else { "full" })
    Install-ToolScripts -DestinationApp $destinationApp
    Set-Content -LiteralPath (Join-Path $DestinationRoot $LauncherName) -Value $LauncherText -Encoding Default

    $runtimeData = Join-Path $destinationApp ".voice_ime"
    if (Test-Path -LiteralPath $runtimeData) {
        Remove-Item -LiteralPath $runtimeData -Recurse -Force
    }
    $appItem = Get-Item -LiteralPath $destinationApp
    $appItem.Attributes = $appItem.Attributes -bor [System.IO.FileAttributes]::Hidden
    Assert-PortableLayout -DestinationRoot $DestinationRoot -CorePackage:$CorePackage
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
Copy-Item -LiteralPath (Join-Path $Root "docs\release-notes-2.0.1.md") -Destination $AppRoot
Copy-Item -LiteralPath (Join-Path $Root "docs\translation-benchmark.md") -Destination $AppRoot
Copy-Item -LiteralPath (Join-Path $Root "docs\hotwords.md") -Destination $AppRoot
if (Test-Path -LiteralPath (Join-Path $Root "CHANGELOG.md")) {
    Copy-Item -LiteralPath (Join-Path $Root "CHANGELOG.md") -Destination $AppRoot
}
$launcherText = @'
@echo off
setlocal
cd /d "%~dp0app"
set "VOICE_IME_ROOT=%~dp0app"
attrib +h "%~dp0app" >nul 2>nul
if not exist "%~dp0app\VoiceIME.exe" (
  echo VoiceIME.exe not found.
  pause
  exit /b 1
)
start "" "%~dp0app\VoiceIME.exe"
'@
Set-Content -LiteralPath (Join-Path $ReleaseRoot $LauncherName) -Value $launcherText -Encoding Default

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
Write-BuildStamp -DestinationApp $AppRoot -PackageName "full"
Install-ToolScripts -DestinationApp $AppRoot

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
Assert-PortableLayout -DestinationRoot $ReleaseRoot

Write-Host "Portable release created: $ReleaseRoot"
Copy-PortableBody -DestinationRoot $CoreReleaseRoot -LauncherText $launcherText -CorePackage
Write-Host "Core portable release created: $CoreReleaseRoot"
}
finally {
    if (Test-Path -LiteralPath $PreserveRoot) {
        Remove-Item -LiteralPath $PreserveRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
