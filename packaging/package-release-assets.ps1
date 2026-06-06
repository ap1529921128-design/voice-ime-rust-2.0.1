param(
    [string]$ReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable",
    [string]$CoreReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core",
    [string]$OutputRoot = "D:\voice-ime-build-release",
    [string]$Version = "",
    [switch]$SkipModelPacks
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$LauncherName = ([string][char]21551 + [string][char]21160 + [string][char]35821 + [string][char]38899 + [string][char]36755 + [string][char]20837 + ".bat")

if ([string]::IsNullOrWhiteSpace($Version)) {
    $packageJson = Get-Content -LiteralPath (Join-Path $Root "package.json") -Raw | ConvertFrom-Json
    $Version = [string]$packageJson.version
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Get-RelativeZipName {
    param(
        [Parameter(Mandatory = $true)][string]$RootPath,
        [Parameter(Mandatory = $true)][string]$FullName
    )
    return $FullName.Substring($RootPath.Length).TrimStart([char]'\', [char]'/').Replace('\', '/')
}

function Assert-PortableSource {
    param(
        [Parameter(Mandatory = $true)][string]$PackageRoot,
        [switch]$CorePackage
    )
    $appDir = Join-Path $PackageRoot "app"
    $launcher = Join-Path $PackageRoot $LauncherName
    if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
        throw "launcher missing: $launcher"
    }
    if (-not (Test-Path -LiteralPath $appDir -PathType Container)) {
        throw "app directory missing: $appDir"
    }
    foreach ($required in @("VoiceIME.exe", "README.md", "BUILD.txt", "models\MODELS.json", "models\MODELS.md")) {
        $path = Join-Path $appDir $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "required package file missing: $required"
        }
    }
    $forbidden = @(Get-ChildItem -LiteralPath $PackageRoot -Force -Recurse -Directory |
        Where-Object { $_.Name -in @(".voice_ime", "recordings", "backup", "backups") })
    if ($forbidden.Count -gt 0) {
        throw "forbidden runtime directories found: $($forbidden.FullName -join '; ')"
    }
    if ($CorePackage) {
        $modelsDir = Join-Path $appDir "models"
        $unexpected = @(Get-ChildItem -LiteralPath $modelsDir -Force |
            Where-Object { $_.Name -notin @("MODELS.json", "MODELS.md") })
        if ($unexpected.Count -gt 0) {
            throw "core models directory is not clean: $($unexpected.Name -join ', ')"
        }
    }
}

function New-ZipFromDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [Parameter(Mandatory = $true)][string]$ZipPath
    )
    if (Test-Path -LiteralPath $ZipPath) {
        Remove-Item -LiteralPath $ZipPath -Force
    }
    $source = (Resolve-Path $SourceRoot).Path.TrimEnd([char]'\', [char]'/')
    $zipFile = [System.IO.File]::Open($ZipPath, [System.IO.FileMode]::CreateNew)
    try {
        $archive = [System.IO.Compression.ZipArchive]::new($zipFile, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            foreach ($dir in Get-ChildItem -LiteralPath $source -Force -Recurse -Directory | Sort-Object FullName) {
                $relative = Get-RelativeZipName -RootPath $source -FullName $dir.FullName
                if ([string]::IsNullOrWhiteSpace($relative)) {
                    continue
                }
                $entry = $archive.CreateEntry($relative + "/")
                $entry.LastWriteTime = $dir.LastWriteTime
                $entry.ExternalAttributes = [int]$dir.Attributes
            }
            foreach ($file in Get-ChildItem -LiteralPath $source -Force -Recurse -File | Sort-Object FullName) {
                $relative = Get-RelativeZipName -RootPath $source -FullName $file.FullName
                $entry = $archive.CreateEntry($relative, [System.IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = $file.LastWriteTime
                $entry.ExternalAttributes = [int]$file.Attributes
                $input = [System.IO.File]::OpenRead($file.FullName)
                try {
                    $output = $entry.Open()
                    try {
                        $input.CopyTo($output)
                    }
                    finally {
                        $output.Dispose()
                    }
                }
                finally {
                    $input.Dispose()
                }
            }
        }
        finally {
            $archive.Dispose()
        }
    }
    finally {
        $zipFile.Dispose()
    }
}

function Assert-PortableZip {
    param(
        [Parameter(Mandatory = $true)][string]$ZipPath,
        [switch]$CorePackage
    )
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $names = @($zip.Entries | ForEach-Object { $_.FullName })
        foreach ($required in @($LauncherName, "app/VoiceIME.exe", "app/README.md", "app/BUILD.txt", "app/models/MODELS.json", "app/models/MODELS.md")) {
            if ($names -notcontains $required) {
                throw "zip missing required entry $required in $ZipPath"
            }
        }
        $rootNames = @($names | ForEach-Object { ($_ -split "/")[0] } | Where-Object { $_ } | Sort-Object -Unique)
        $unexpectedRoot = @($rootNames | Where-Object { $_ -notin @("app", $LauncherName) })
        if ($unexpectedRoot.Count -gt 0) {
            throw "zip has unexpected root entries: $($unexpectedRoot -join ', ')"
        }
        $forbidden = @($names | Where-Object {
                $_ -match '(^|/)\.voice_ime(/|$)' -or
                $_ -match '(^|/)recordings(/|$)' -or
                $_ -match '(^|/)backups?(/|$)'
            })
        if ($forbidden.Count -gt 0) {
            throw "zip contains runtime or backup data: $($forbidden -join ', ')"
        }
        if ($CorePackage) {
            $unexpectedModels = @($names |
                Where-Object { $_ -like "app/models/*" -and $_ -notin @("app/models/", "app/models/MODELS.json", "app/models/MODELS.md") })
            if ($unexpectedModels.Count -gt 0) {
                throw "core zip contains model binaries or extra files: $($unexpectedModels -join ', ')"
            }
        }
    }
    finally {
        $zip.Dispose()
    }
}

function New-AssetRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Kind,
        [string]$Role = ""
    )
    $item = Get-Item -LiteralPath $Path
    return [ordered]@{
        name     = $item.Name
        kind     = $Kind
        role     = $Role
        path     = $item.FullName
        bytes    = $item.Length
        sha256   = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        modified = $item.LastWriteTime.ToString("o")
    }
}

if (-not (Test-Path -LiteralPath $OutputRoot -PathType Container)) {
    New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
}

Assert-PortableSource -PackageRoot $ReleaseRoot
Assert-PortableSource -PackageRoot $CoreReleaseRoot -CorePackage

$assets = @()

$fullZip = Join-Path $OutputRoot "voice-ime-$Version-rust-portable.zip"
$coreZip = Join-Path $OutputRoot "voice-ime-$Version-rust-portable-core.zip"

New-ZipFromDirectory -SourceRoot $ReleaseRoot -ZipPath $fullZip
Assert-PortableZip -ZipPath $fullZip
$assets += New-AssetRecord -Path $fullZip -Kind "portable" -Role "full"

New-ZipFromDirectory -SourceRoot $CoreReleaseRoot -ZipPath $coreZip
Assert-PortableZip -ZipPath $coreZip -CorePackage
$assets += New-AssetRecord -Path $coreZip -Kind "portable" -Role "core"

if (-not $SkipModelPacks) {
    foreach ($modelPack in Get-ChildItem -LiteralPath $OutputRoot -Filter "voice-ime-model-pack-*.zip" -File | Sort-Object Name) {
        $assets += New-AssetRecord -Path $modelPack.FullName -Kind "model-pack"
    }
    foreach ($manifestName in @("voice-ime-model-packs-$Version.json", "voice-ime-model-packs-$Version.md")) {
        $manifestPath = Join-Path $OutputRoot $manifestName
        if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
            $assets += New-AssetRecord -Path $manifestPath -Kind "model-pack-manifest"
        }
    }
}

$releaseManifest = [ordered]@{
    schema_version = 1
    app_version    = $Version
    tag            = "v$Version"
    repository     = "ap1529921128-design/voice-ime-rust-2.0.1"
    created_at     = (Get-Date).ToString("o")
    source_root    = $Root.Path
    assets         = $assets
}

$jsonPath = Join-Path $OutputRoot "voice-ime-release-assets-$Version.json"
$mdPath = Join-Path $OutputRoot "voice-ime-release-assets-$Version.md"
Write-Utf8NoBom -Path $jsonPath -Text ($releaseManifest | ConvertTo-Json -Depth 8)

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("# Voice IME Release Assets $Version")
$lines.Add("")
$lines.Add("- Tag: ``v$Version``")
$lines.Add("- Repository: ``ap1529921128-design/voice-ime-rust-2.0.1``")
$lines.Add("- Created: $($releaseManifest.created_at)")
$lines.Add("")
$lines.Add("| asset | kind | role | MB | sha256 |")
$lines.Add("| --- | --- | --- | ---: | --- |")
foreach ($asset in $assets) {
    $mb = [Math]::Round(([int64]$asset.bytes) / 1MB, 1)
    $sha = ([string]$asset.sha256).Substring(0, 16)
    $lines.Add("| ``$($asset.name)`` | $($asset.kind) | $($asset.role) | $mb | ``$sha...`` |")
}
$lines.Add("")
$lines.Add("Suggested validation and upload command after installing/authenticating GitHub CLI:")
$lines.Add("")
$uploadPaths = @($assets | ForEach-Object { $_.path }) + @($jsonPath, $mdPath)
$assetArgs = ($uploadPaths | ForEach-Object { '"' + $_ + '"' }) -join " "
$lines.Add('```powershell')
$lines.Add(".\packaging\publish-github-release.ps1 -AssetsManifest ""$jsonPath"" -ValidateOnly")
$lines.Add(".\packaging\publish-github-release.ps1 -AssetsManifest ""$jsonPath""")
$lines.Add("")
$lines.Add("# Or use gh directly:")
$lines.Add("gh release create v$Version $assetArgs --repo ap1529921128-design/voice-ime-rust-2.0.1 --title ""Voice IME Rust $Version"" --notes-file ""docs/release-notes-$Version.md""")
$lines.Add('```')
$lines.Add("")
$lines.Add("Suggested validation and upload command without GitHub CLI:")
$lines.Add("")
$lines.Add('```powershell')
$lines.Add('$env:GH_TOKEN = "<github-token-with-repo-scope>"')
$lines.Add(".\packaging\publish-github-release.ps1 -AssetsManifest ""$jsonPath"" -ValidateOnly")
$lines.Add(".\packaging\publish-github-release.ps1 -AssetsManifest ""$jsonPath""")
$lines.Add('```')
Write-Utf8NoBom -Path $mdPath -Text ($lines -join [Environment]::NewLine)

Write-Host "Release assets packaged:"
foreach ($asset in $assets) {
    Write-Host " - $($asset.name) $([Math]::Round(([int64]$asset.bytes) / 1MB, 1)) MB"
}
Write-Host " - $(Split-Path -Leaf $jsonPath) manifest"
Write-Host " - $(Split-Path -Leaf $mdPath) summary"
Write-Host "Manifest: $jsonPath"
Write-Host "Summary: $mdPath"
