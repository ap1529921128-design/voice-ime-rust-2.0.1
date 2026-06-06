param(
    [string]$ModelRoot = "",
    [switch]$Clear,
    [switch]$Open
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ModelRootFile = Join-Path $AppDir "MODEL_ROOT.txt"
$PackagedModelsDir = Join-Path $AppDir "models"
$RuntimeDir = Join-Path $AppDir ".voice_ime"
$LogsDir = Join-Path $RuntimeDir "logs"
$ConfigPath = Join-Path $RuntimeDir "config.json"
$ReportPath = Join-Path $LogsDir ("model-root-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")

New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null

function Resolve-PortablePath {
    param([Parameter(Mandatory = $true)][string]$Value)

    $trimmed = $Value.Trim()
    if ([string]::IsNullOrWhiteSpace($trimmed)) {
        return ""
    }
    if ([System.IO.Path]::IsPathRooted($trimmed)) {
        return [System.IO.Path]::GetFullPath($trimmed)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $AppDir $trimmed))
}

function Read-ModelRootFileValue {
    if (-not (Test-Path -LiteralPath $ModelRootFile -PathType Leaf)) {
        return ""
    }
    foreach ($line in Get-Content -LiteralPath $ModelRootFile -ErrorAction SilentlyContinue) {
        $value = ([string]$line).Trim().TrimStart([char]0xfeff).Trim()
        if (-not [string]::IsNullOrWhiteSpace($value) -and -not $value.StartsWith("#")) {
            return $value
        }
    }
    return ""
}

function Read-ConfiguredModelRoot {
    if (-not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
        return ""
    }
    try {
        $config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
        if ($config.asr -and $config.asr.model_root) {
            return [string]$config.asr.model_root
        }
    }
    catch {
        return ""
    }
    return ""
}

function Get-EffectiveModelRoot {
    $envRoot = [Environment]::GetEnvironmentVariable("VOICE_IME_MODEL_DIR", "Process")
    if (-not [string]::IsNullOrWhiteSpace($envRoot)) {
        return [pscustomobject]@{ source = "VOICE_IME_MODEL_DIR"; root = (Resolve-PortablePath -Value $envRoot); value = $envRoot }
    }
    $fileRoot = Read-ModelRootFileValue
    if (-not [string]::IsNullOrWhiteSpace($fileRoot)) {
        return [pscustomobject]@{ source = "MODEL_ROOT.txt"; root = (Resolve-PortablePath -Value $fileRoot); value = $fileRoot }
    }
    $configured = Read-ConfiguredModelRoot
    if (-not [string]::IsNullOrWhiteSpace($configured) -and $configured.Trim() -ne "models") {
        return [pscustomobject]@{ source = "asr.model_root"; root = (Resolve-PortablePath -Value $configured); value = $configured }
    }
    return [pscustomobject]@{ source = "default"; root = $PackagedModelsDir; value = "models" }
}

function Get-ManifestPath {
    param([Parameter(Mandatory = $true)][string]$EffectiveRoot)

    $effectiveManifest = Join-Path $EffectiveRoot "MODELS.json"
    if (Test-Path -LiteralPath $effectiveManifest -PathType Leaf) {
        return [pscustomobject]@{ source = "effective"; path = $effectiveManifest }
    }
    $packagedManifest = Join-Path $PackagedModelsDir "MODELS.json"
    if (Test-Path -LiteralPath $packagedManifest -PathType Leaf) {
        return [pscustomobject]@{ source = "packaged"; path = $packagedManifest }
    }
    return [pscustomobject]@{ source = "missing"; path = "" }
}

function ConvertTo-ModelRootRelativePath {
    param([Parameter(Mandatory = $true)][string]$TargetDir)

    $normalized = $TargetDir -replace "\\", "/"
    if ($normalized -eq "app/models" -or $normalized -eq "models") {
        return ""
    }
    if ($normalized.StartsWith("app/models/")) {
        return $normalized.Substring("app/models/".Length)
    }
    if ($normalized.StartsWith("models/")) {
        return $normalized.Substring("models/".Length)
    }
    return $normalized
}

function Get-PackRows {
    param(
        [Parameter(Mandatory = $true)][string]$EffectiveRoot,
        [Parameter(Mandatory = $true)][string]$ManifestPath
    )

    $rows = [System.Collections.Generic.List[object]]::new()
    if ([string]::IsNullOrWhiteSpace($ManifestPath) -or -not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
        return $rows
    }
    $manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
    foreach ($pack in $manifest.packs) {
        $relativeTarget = ConvertTo-ModelRootRelativePath -TargetDir ([string]$pack.target_dir)
        $targetDir = if ([string]::IsNullOrWhiteSpace($relativeTarget)) {
            $EffectiveRoot
        }
        else {
            Join-Path $EffectiveRoot ($relativeTarget -replace '/', [System.IO.Path]::DirectorySeparatorChar)
        }
        $missing = [System.Collections.Generic.List[string]]::new()
        foreach ($required in $pack.required_files) {
            $file = Join-Path $targetDir ([string]$required)
            if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
                $missing.Add([string]$required) | Out-Null
            }
        }
        $state = "READY"
        if ([string]$pack.status -eq "planned") {
            $state = "PLANNED"
        }
        elseif ($missing.Count -gt 0) {
            $state = "MISSING"
        }
        $rows.Add([pscustomobject]@{
            state     = $state
            id        = [string]$pack.id
            kind      = [string]$pack.kind
            profile   = [string]$pack.profile
            status    = [string]$pack.status
            target    = $targetDir
            missing   = ($missing -join ",")
            estimated = [string]$pack.estimated_size_mb
        }) | Out-Null
    }
    return $rows
}

if ($Clear -and -not [string]::IsNullOrWhiteSpace($ModelRoot)) {
    throw "Use either -ModelRoot or -Clear, not both."
}

if ($Clear) {
    if (Test-Path -LiteralPath $ModelRootFile -PathType Leaf) {
        Remove-Item -LiteralPath $ModelRootFile -Force
        Write-Host "Cleared MODEL_ROOT.txt"
    }
    else {
        Write-Host "MODEL_ROOT.txt was already absent"
    }
}
elseif (-not [string]::IsNullOrWhiteSpace($ModelRoot)) {
    $target = Resolve-PortablePath -Value $ModelRoot
    New-Item -ItemType Directory -Path $target -Force | Out-Null
    $body = @(
        "# Voice IME portable model root",
        "# This file is read before .voice_ime/config.json.",
        $target
    )
    Set-Content -LiteralPath $ModelRootFile -Value ($body -join [Environment]::NewLine) -Encoding UTF8
    Write-Host "MODEL_ROOT.txt -> $target"
}

$effective = Get-EffectiveModelRoot
$manifest = Get-ManifestPath -EffectiveRoot $effective.root
$rows = Get-PackRows -EffectiveRoot $effective.root -ManifestPath $manifest.path
$ready = @($rows | Where-Object { $_.state -eq "READY" })
$missing = @($rows | Where-Object { $_.state -eq "MISSING" })
$planned = @($rows | Where-Object { $_.state -eq "PLANNED" })

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("Voice IME Model Root")
$lines.Add("created_at=$((Get-Date).ToString("o"))")
$lines.Add("app_dir=$AppDir")
$lines.Add("model_root_file=$ModelRootFile")
$lines.Add("model_root_file_value=$(Read-ModelRootFileValue)")
$lines.Add("config_path=$ConfigPath")
$lines.Add("configured_model_root=$(Read-ConfiguredModelRoot)")
$lines.Add("env_model_root=$([Environment]::GetEnvironmentVariable("VOICE_IME_MODEL_DIR", "Process"))")
$lines.Add("effective_source=$($effective.source)")
$lines.Add("effective_root=$($effective.root)")
$lines.Add("effective_exists=$(Test-Path -LiteralPath $effective.root -PathType Container)")
$lines.Add("manifest_source=$($manifest.source)")
$lines.Add("manifest_path=$($manifest.path)")
$lines.Add("ready_count=$($ready.Count)")
$lines.Add("missing_count=$($missing.Count)")
$lines.Add("planned_count=$($planned.Count)")
$lines.Add("")
foreach ($row in $rows) {
    $detail = "target=$($row.target)"
    if (-not [string]::IsNullOrWhiteSpace($row.missing)) {
        $detail = "$detail missing=$($row.missing)"
    }
    $lines.Add("$($row.state)`t$($row.kind)`t$($row.profile)`t$($row.id)`t$detail")
}

Set-Content -LiteralPath $ReportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
Write-Host ($lines -join [Environment]::NewLine)
Write-Host "Report: $ReportPath"

if ($Open -and (Test-Path -LiteralPath $effective.root -PathType Container)) {
    Start-Process -FilePath "explorer.exe" -ArgumentList $effective.root | Out-Null
}
