param(
    [string]$SourceModelsDir = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable\app\models",
    [string]$OutputRoot = "D:\voice-ime-build-release",
    [string[]]$Profiles = @(),
    [switch]$IncludePlanned,
    [switch]$FailOnMissing
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$ManifestPath = Join-Path $Root "packaging\model-manifest.json"
$PackScript = Join-Path $Root "packaging\package-model-pack.ps1"

if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Model manifest missing: $ManifestPath"
}
if (-not (Test-Path -LiteralPath $PackScript -PathType Leaf)) {
    throw "Model pack script missing: $PackScript"
}
if (-not (Test-Path -LiteralPath $SourceModelsDir -PathType Container)) {
    throw "Source models dir missing: $SourceModelsDir"
}
if (-not (Test-Path -LiteralPath $OutputRoot -PathType Container)) {
    New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Get-TargetRelativePath {
    param([Parameter(Mandatory = $true)][string]$TargetDir)
    return [regex]::Replace($TargetDir, '^app[\\/]+models[\\/]*', '', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
}

function Get-PackSourceDir {
    param([Parameter(Mandatory = $true)]$Pack)
    $targetRelative = Get-TargetRelativePath -TargetDir ([string]$Pack.target_dir)
    if ([string]::IsNullOrWhiteSpace($targetRelative)) {
        return $SourceModelsDir
    }
    return Join-Path $SourceModelsDir $targetRelative
}

function Get-MissingRequiredFiles {
    param([Parameter(Mandatory = $true)]$Pack)
    $sourcePackDir = Get-PackSourceDir -Pack $Pack
    $missing = [System.Collections.Generic.List[string]]::new()
    if (-not (Test-Path -LiteralPath $sourcePackDir -PathType Container)) {
        $missing.Add($sourcePackDir)
        return $missing
    }
    foreach ($file in $Pack.required_files) {
        $path = Join-Path $sourcePackDir ([string]$file)
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            $missing.Add($path)
        }
    }
    return $missing
}

function Get-ZipMetadataSummary {
    param([Parameter(Mandatory = $true)][string]$ZipPath)
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $metadataEntry = $zip.Entries | Where-Object { $_.FullName -eq "MODEL_PACK.json" } | Select-Object -First 1
        if (-not $metadataEntry) {
            throw "MODEL_PACK.json missing from $ZipPath"
        }
        $reader = [System.IO.StreamReader]::new($metadataEntry.Open())
        try {
            $metadata = $reader.ReadToEnd() | ConvertFrom-Json
        }
        finally {
            $reader.Dispose()
        }
        return [ordered]@{
            metadata_id         = [string]$metadata.id
            metadata_profile    = [string]$metadata.profile
            metadata_kind       = [string]$metadata.kind
            metadata_file_count = @($metadata.files).Count
        }
    }
    finally {
        $zip.Dispose()
    }
}

$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$profileSet = @{}
foreach ($profile in $Profiles) {
    if (-not [string]::IsNullOrWhiteSpace($profile)) {
        $profileSet[$profile.ToLowerInvariant()] = $true
    }
}

$packs = @($manifest.packs | Where-Object {
    $id = ([string]$_.id).ToLowerInvariant()
    $profile = ([string]$_.profile).ToLowerInvariant()
    $explicit = $profileSet.Count -gt 0
    if ($explicit) {
        return $profileSet.ContainsKey($id) -or $profileSet.ContainsKey($profile)
    }
    return $IncludePlanned -or ([string]$_.status -ne "planned")
})

if ($packs.Count -eq 0) {
    throw "No model packs matched the requested profile/id filter."
}

$results = [System.Collections.Generic.List[object]]::new()
foreach ($pack in $packs) {
    $safeId = ([string]$pack.id) -replace '[\\/:*?"<>|]', '-'
    $zipPath = Join-Path $OutputRoot "voice-ime-model-pack-$safeId.zip"
    $missing = @(Get-MissingRequiredFiles -Pack $pack)
    if ($missing.Count -gt 0) {
        $results.Add([ordered]@{
            id               = [string]$pack.id
            profile          = [string]$pack.profile
            kind             = [string]$pack.kind
            manifest_status  = [string]$pack.status
            status           = "missing"
            source_dir       = Get-PackSourceDir -Pack $pack
            target_dir       = [string]$pack.target_dir
            missing_files    = $missing
            zip_path         = ""
            zip_bytes        = 0
            zip_sha256       = ""
            metadata_files   = 0
        })
        Write-Warning "Skipping $($pack.id); missing: $($missing -join '; ')"
        continue
    }

    & powershell -NoProfile -ExecutionPolicy Bypass -File $PackScript `
        -Profile ([string]$pack.id) `
        -SourceModelsDir $SourceModelsDir `
        -OutputRoot $OutputRoot
    if ($LASTEXITCODE -ne 0) {
        throw "package-model-pack.ps1 failed for $($pack.id) with exit code $LASTEXITCODE"
    }
    if (-not (Test-Path -LiteralPath $zipPath -PathType Leaf)) {
        throw "Expected model pack zip was not created: $zipPath"
    }
    $zip = Get-Item -LiteralPath $zipPath
    $metadataSummary = Get-ZipMetadataSummary -ZipPath $zipPath
    $results.Add([ordered]@{
        id               = [string]$pack.id
        profile          = [string]$pack.profile
        kind             = [string]$pack.kind
        manifest_status  = [string]$pack.status
        status           = "created"
        source_dir       = Get-PackSourceDir -Pack $pack
        target_dir       = [string]$pack.target_dir
        missing_files    = @()
        zip_path         = $zip.FullName
        zip_bytes        = $zip.Length
        zip_sha256       = (Get-FileHash -LiteralPath $zip.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        metadata_files   = [int]$metadataSummary.metadata_file_count
    })
}

$releaseManifest = [ordered]@{
    schema_version    = 1
    app_version       = [string]$manifest.app_version
    source_models_dir = (Resolve-Path -LiteralPath $SourceModelsDir).Path
    output_root       = (Resolve-Path -LiteralPath $OutputRoot).Path
    created_at        = (Get-Date).ToString("o")
    packs             = $results
}
$jsonPath = Join-Path $OutputRoot "voice-ime-model-packs-$($manifest.app_version).json"
$releaseManifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

$mdPath = Join-Path $OutputRoot "voice-ime-model-packs-$($manifest.app_version).md"
$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("# Voice IME Model Pack Release")
$lines.Add("")
$lines.Add("- App version: $($manifest.app_version)")
$lines.Add("- Source models: $SourceModelsDir")
$lines.Add("- Created: $($releaseManifest.created_at)")
$lines.Add("")
$lines.Add("| id | profile | kind | status | zip MB | sha256 |")
$lines.Add("| --- | --- | --- | --- | ---: | --- |")
foreach ($result in $results) {
    $mb = if ($result.zip_bytes -gt 0) { [Math]::Round($result.zip_bytes / 1MB, 1) } else { 0 }
    $sha = if ($result.zip_sha256) { $result.zip_sha256.Substring(0, 12) } else { "" }
    $lines.Add("| $($result.id) | $($result.profile) | $($result.kind) | $($result.status) | $mb | $sha |")
}
$lines.Add("")
$lines.Add("Full SHA-256 values and missing-file details are in ``$(Split-Path -Leaf $jsonPath)``.")
Set-Content -LiteralPath $mdPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8

$created = @($results | Where-Object { $_.status -eq "created" }).Count
$missing = @($results | Where-Object { $_.status -eq "missing" }).Count
Write-Host "Model pack batch complete: created=$created missing=$missing"
Write-Host "Manifest: $jsonPath"
Write-Host "Summary: $mdPath"

if ($FailOnMissing -and $missing -gt 0) {
    throw "Some requested model packs are missing required files. See $jsonPath"
}
