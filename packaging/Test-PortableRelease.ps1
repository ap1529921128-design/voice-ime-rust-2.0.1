param(
    [string]$ReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable",
    [string]$CoreReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core",
    [string]$ModelPackZip = "D:\voice-ime-build-release\voice-ime-model-pack-asr-fallback-whisper-tiny-int8.zip",
    [switch]$SkipNotepad,
    [switch]$SkipBrowser,
    [switch]$SkipTranslation,
    [switch]$SkipModelPackImport,
    [switch]$KeepRuntimeData
)

$ErrorActionPreference = "Stop"

$LauncherName = ([string][char]21551 + [string][char]21160 + [string][char]35821 + [string][char]38899 + [string][char]36755 + [string][char]20837 + ".bat")

function Assert-PortableLayout {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [switch]$CorePackage
    )

    $resolvedRoot = Resolve-Path -LiteralPath $Root
    $app = Join-Path $resolvedRoot "app"
    $launcher = Join-Path $resolvedRoot $LauncherName
    if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
        throw "Launcher missing: $launcher"
    }
    if (-not (Test-Path -LiteralPath $app -PathType Container)) {
        throw "Hidden app directory missing: $app"
    }
    $appItem = Get-Item -LiteralPath $app -Force
    if (-not (($appItem.Attributes -band [System.IO.FileAttributes]::Hidden) -eq [System.IO.FileAttributes]::Hidden)) {
        throw "App directory is not hidden: $app"
    }
    $visibleRootItems = @(Get-ChildItem -LiteralPath $resolvedRoot)
    if ($visibleRootItems.Count -ne 1 -or $visibleRootItems[0].Name -ne $LauncherName) {
        throw "Root must visibly expose only $LauncherName"
    }
    foreach ($required in @("VoiceIME.exe", "BUILD.txt", "README.md", "models\MODELS.json", "models\MODELS.md")) {
        $path = Join-Path $app $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Required app file missing: $required"
        }
    }

    if ($CorePackage) {
        $modelsDir = Join-Path $app "models"
        $unexpected = @(Get-ChildItem -LiteralPath $modelsDir -Force | Where-Object { $_.Name -notin @("MODELS.json", "MODELS.md") })
        if ($unexpected.Count -gt 0) {
            throw "Core package models directory contains binaries or extra files: $($unexpected.Name -join ', ')"
        }
    }
}

function Assert-BuildStamp {
    param([Parameter(Mandatory = $true)][string]$Root)

    $build = Join-Path $Root "app\BUILD.txt"
    $body = Get-Content -LiteralPath $build -Raw
    foreach ($needle in @("version=2.0.1", "git_status=clean", "git_commit=")) {
        if (-not $body.Contains($needle)) {
            throw "BUILD.txt missing '$needle'"
        }
    }
}

function Invoke-StartupSmoke {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-$Name-startup-" + [guid]::NewGuid().ToString("N"))
        $process = Start-Process -FilePath $exe -WorkingDirectory $app -WindowStyle Hidden -PassThru
        Start-Sleep -Seconds 5
        if ($process.HasExited) {
            throw "$Name startup exited early with code $($process.ExitCode)"
        }
        Stop-Process -Id $process.Id -Force
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    Write-Host "$Name startup smoke passed"
}

function Invoke-DoctorSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-doctor-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList "--doctor" -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Doctor exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "doctor-*.txt" -File -ErrorAction SilentlyContinue)
    if ($reports.Count -eq 0) {
        throw "Doctor did not write a report under $appDir"
    }
    $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $body = Get-Content -LiteralPath $report.FullName -Raw
    if (-not $body.Contains("本地 LLM 文件")) {
        throw "Doctor report does not include local LLM file check"
    }
    Write-Host "Doctor smoke passed: $($report.FullName)"
}

function Invoke-AcceptanceScript {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$ScriptName
    )

    $script = Join-Path $Root "app\tools\$ScriptName"
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "Acceptance script missing: $script"
    }
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        & powershell -NoProfile -ExecutionPolicy Bypass -File $script
        if ($LASTEXITCODE -ne 0) {
            throw "$ScriptName failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
}

function Invoke-ModelPackImportAcceptance {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$CoreRoot,
        [Parameter(Mandatory = $true)][string]$PackZip
    )

    $script = Join-Path $Root "app\tools\Model-Pack-Import-Acceptance.ps1"
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "Acceptance script missing: $script"
    }
    & powershell -NoProfile -ExecutionPolicy Bypass -File $script `
        -CoreReleaseRoot $CoreRoot `
        -ModelPackZip $PackZip
    if ($LASTEXITCODE -ne 0) {
        throw "Model-Pack-Import-Acceptance.ps1 failed with exit code $LASTEXITCODE"
    }
}

function Remove-PackageRuntimeData {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Resolve-Path -LiteralPath (Join-Path $Root "app")
    $target = Join-Path $app ".voice_ime"
    if (-not (Test-Path -LiteralPath $target)) {
        return
    }
    $resolved = Resolve-Path -LiteralPath $target
    if (-not $resolved.Path.StartsWith($app.Path, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove runtime data outside package app: $($resolved.Path)"
    }
    Remove-Item -LiteralPath $resolved.Path -Recurse -Force
    Write-Host "Removed package runtime data: $($resolved.Path)"
}

if (-not $KeepRuntimeData) {
    Remove-PackageRuntimeData -Root $ReleaseRoot
    Remove-PackageRuntimeData -Root $CoreReleaseRoot
}

Assert-PortableLayout -Root $ReleaseRoot
Assert-PortableLayout -Root $CoreReleaseRoot -CorePackage
Assert-BuildStamp -Root $ReleaseRoot
Assert-BuildStamp -Root $CoreReleaseRoot
Invoke-StartupSmoke -Root $ReleaseRoot -Name "full"
Invoke-StartupSmoke -Root $CoreReleaseRoot -Name "core"
Invoke-DoctorSmoke -Root $ReleaseRoot
if (-not $SkipNotepad) {
    Invoke-AcceptanceScript -Root $ReleaseRoot -ScriptName "Notepad-Input-Acceptance.ps1"
}
if (-not $SkipBrowser) {
    Invoke-AcceptanceScript -Root $ReleaseRoot -ScriptName "Browser-Input-Acceptance.ps1"
}
if (-not $SkipTranslation) {
    Invoke-AcceptanceScript -Root $ReleaseRoot -ScriptName "Translation-Acceptance.ps1"
}
if (-not $SkipModelPackImport) {
    Invoke-ModelPackImportAcceptance -Root $ReleaseRoot -CoreRoot $CoreReleaseRoot -PackZip $ModelPackZip
}
if (-not $KeepRuntimeData) {
    Remove-PackageRuntimeData -Root $ReleaseRoot
    Remove-PackageRuntimeData -Root $CoreReleaseRoot
}

Write-Host "Portable release verification passed"
