param(
    [Parameter(Mandatory = $true)][string]$Profile,
    [string]$SourceModelsDir = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable\app\models",
    [string]$OutputRoot = "D:\voice-ime-build-release"
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$ManifestPath = Join-Path $Root "packaging\model-manifest.json"

if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Model manifest missing: $ManifestPath"
}
if (-not (Test-Path -LiteralPath $SourceModelsDir -PathType Container)) {
    throw "Source models dir missing: $SourceModelsDir"
}

$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
$pack = @($manifest.packs | Where-Object { $_.profile -eq $Profile -or $_.id -eq $Profile } | Select-Object -First 1)
if ($pack.Count -eq 0) {
    $known = ($manifest.packs | ForEach-Object { "$($_.profile)/$($_.id)" }) -join ", "
    throw "Unknown model profile or id: $Profile. Known: $known"
}
$pack = $pack[0]

$targetDir = [string]$pack.target_dir
$targetRelative = $targetDir -replace '^app[\\/]+models[\\/]*', ''
$sourcePackDir = if ([string]::IsNullOrWhiteSpace($targetRelative)) {
    $SourceModelsDir
}
else {
    Join-Path $SourceModelsDir $targetRelative
}
if (-not (Test-Path -LiteralPath $sourcePackDir -PathType Container)) {
    throw "Model pack source dir missing: $sourcePackDir"
}

$safeId = ([string]$pack.id) -replace '[\\/:*?"<>|]', '-'
$zipPath = Join-Path $OutputRoot "voice-ime-model-pack-$safeId.zip"
$staging = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-model-pack-" + [guid]::NewGuid().ToString("N"))

try {
    $stageModels = Join-Path $staging "app\models"
    $stageTarget = if ([string]::IsNullOrWhiteSpace($targetRelative)) {
        $stageModels
    }
    else {
        Join-Path $stageModels $targetRelative
    }
    New-Item -ItemType Directory -Path $stageTarget -Force | Out-Null

    foreach ($file in $pack.required_files) {
        $source = Join-Path $sourcePackDir ([string]$file)
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Required model file missing: $source"
        }
        $destination = Join-Path $stageTarget ([string]$file)
        $destinationParent = Split-Path -Parent $destination
        if (-not (Test-Path -LiteralPath $destinationParent -PathType Container)) {
            New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
        }
        Copy-Item -LiteralPath $source -Destination $destination -Force
    }

    Copy-Item -LiteralPath $ManifestPath -Destination (Join-Path $stageModels "MODELS.json") -Force
    $summary = @(
        "Voice IME Model Pack",
        "id=$($pack.id)",
        "profile=$($pack.profile)",
        "kind=$($pack.kind)",
        "target_dir=$($pack.target_dir)",
        "source_models_dir=$SourceModelsDir",
        "created_at=$((Get-Date).ToString("o"))"
    )
    Set-Content -LiteralPath (Join-Path $staging "MODEL_PACK.txt") -Value ($summary -join [Environment]::NewLine) -Encoding UTF8

    $metadataFiles = @()
    foreach ($file in Get-ChildItem -LiteralPath $staging -Recurse -File | Sort-Object FullName) {
        if ($file.Name -eq "MODEL_PACK.json") {
            continue
        }
        $relative = $file.FullName.Substring($staging.Length).TrimStart([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
        $relative = $relative -replace '\\', '/'
        $metadataFiles += [ordered]@{
            path   = $relative
            bytes  = $file.Length
            sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $metadata = [ordered]@{
        schema_version    = 1
        app_version       = [string]$manifest.app_version
        id                = [string]$pack.id
        profile           = [string]$pack.profile
        kind              = [string]$pack.kind
        target_dir        = [string]$pack.target_dir
        source_models_dir = $SourceModelsDir
        created_at        = (Get-Date).ToString("o")
        files             = $metadataFiles
    }
    $metadataJson = $metadata | ConvertTo-Json -Depth 8
    [System.IO.File]::WriteAllText(
        (Join-Path $staging "MODEL_PACK.json"),
        $metadataJson,
        [System.Text.UTF8Encoding]::new($false)
    )

    if (-not (Test-Path -LiteralPath $OutputRoot -PathType Container)) {
        New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
    }
    Compress-Archive -Path (Join-Path $staging "*") -DestinationPath $zipPath -Force
    Write-Host "Model pack created: $zipPath"
}
finally {
    if (Test-Path -LiteralPath $staging) {
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
    }
}
