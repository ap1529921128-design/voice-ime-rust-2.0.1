param(
    [string]$Repo = "ap1529921128-design/voice-ime-rust-2.0.1",
    [string]$Tag = "v2.0.1",
    [string]$Title = "Voice IME Rust 2.0.1",
    [string]$AssetsManifest = "D:\voice-ime-build-release\voice-ime-release-assets-2.0.1.json",
    [string]$NotesPath = "docs\release-notes-2.0.1.md",
    [string]$Token = ""
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
if (-not [System.IO.Path]::IsPathRooted($NotesPath)) {
    $NotesPath = Join-Path $Root $NotesPath
}

if (-not (Test-Path -LiteralPath $AssetsManifest -PathType Leaf)) {
    throw "Assets manifest missing: $AssetsManifest. Run packaging\package-release-assets.ps1 first."
}
if (-not (Test-Path -LiteralPath $NotesPath -PathType Leaf)) {
    throw "Release notes missing: $NotesPath"
}

$manifest = Get-Content -LiteralPath $AssetsManifest -Raw | ConvertFrom-Json
$assets = @($manifest.assets | Where-Object { Test-Path -LiteralPath $_.path -PathType Leaf })
$summaryPath = [System.IO.Path]::ChangeExtension($AssetsManifest, ".md")
foreach ($extra in @($AssetsManifest, $summaryPath)) {
    if (Test-Path -LiteralPath $extra -PathType Leaf) {
        $item = Get-Item -LiteralPath $extra
        if (-not @($assets | Where-Object { $_.name -eq $item.Name })) {
            $assets += [pscustomobject]@{
                name = $item.Name
                path = $item.FullName
            }
        }
    }
}
if ($assets.Count -eq 0) {
    throw "No uploadable assets found in $AssetsManifest"
}

$gh = Get-Command gh -ErrorAction SilentlyContinue
if ($gh) {
    & gh release view $Tag --repo $Repo *> $null
    if ($LASTEXITCODE -eq 0) {
        & gh release edit $Tag --repo $Repo --title $Title --notes-file $NotesPath
        if ($LASTEXITCODE -ne 0) {
            throw "gh release edit failed"
        }
        & gh release upload $Tag @($assets | ForEach-Object { $_.path }) --repo $Repo --clobber
        if ($LASTEXITCODE -ne 0) {
            throw "gh release upload failed"
        }
    }
    else {
        & gh release create $Tag @($assets | ForEach-Object { $_.path }) --repo $Repo --title $Title --notes-file $NotesPath
        if ($LASTEXITCODE -ne 0) {
            throw "gh release create failed"
        }
    }
    Write-Host "GitHub release published with gh: https://github.com/$Repo/releases/tag/$Tag"
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Token)) {
    if (-not [string]::IsNullOrWhiteSpace($env:GH_TOKEN)) {
        $Token = $env:GH_TOKEN
    }
    elseif (-not [string]::IsNullOrWhiteSpace($env:GITHUB_TOKEN)) {
        $Token = $env:GITHUB_TOKEN
    }
}
if ([string]::IsNullOrWhiteSpace($Token)) {
    throw "GitHub CLI is not installed and no GH_TOKEN/GITHUB_TOKEN was provided. Install gh or pass -Token."
}

$headers = @{
    Authorization          = "Bearer $Token"
    Accept                 = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
}
$apiBase = "https://api.github.com/repos/$Repo"
$notes = Get-Content -LiteralPath $NotesPath -Raw

try {
    $release = Invoke-RestMethod -Method Get -Uri "$apiBase/releases/tags/$Tag" -Headers $headers
    $body = @{
        tag_name = $Tag
        name     = $Title
        body     = $notes
    } | ConvertTo-Json -Depth 4
    $release = Invoke-RestMethod -Method Patch -Uri "$apiBase/releases/$($release.id)" -Headers $headers -Body $body -ContentType "application/json"
}
catch {
    $status = $_.Exception.Response.StatusCode.value__
    if ($status -ne 404) {
        throw
    }
    $body = @{
        tag_name   = $Tag
        target_commitish = "main"
        name       = $Title
        body       = $notes
        draft      = $false
        prerelease = $false
    } | ConvertTo-Json -Depth 4
    $release = Invoke-RestMethod -Method Post -Uri "$apiBase/releases" -Headers $headers -Body $body -ContentType "application/json"
}

$existingAssets = Invoke-RestMethod -Method Get -Uri $release.assets_url -Headers $headers
$uploadBase = $release.upload_url -replace "\{\?name,label\}$", ""

foreach ($asset in $assets) {
    $existing = @($existingAssets | Where-Object { $_.name -eq $asset.name } | Select-Object -First 1)
    if ($existing.Count -gt 0) {
        Invoke-RestMethod -Method Delete -Uri "$apiBase/releases/assets/$($existing[0].id)" -Headers $headers | Out-Null
    }
    $encodedName = [System.Uri]::EscapeDataString([string]$asset.name)
    Invoke-RestMethod `
        -Method Post `
        -Uri "$uploadBase?name=$encodedName" `
        -Headers $headers `
        -ContentType "application/octet-stream" `
        -InFile ([string]$asset.path) | Out-Null
    Write-Host "Uploaded: $($asset.name)"
}

Write-Host "GitHub release published: https://github.com/$Repo/releases/tag/$Tag"
