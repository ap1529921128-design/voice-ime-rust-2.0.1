param(
    [string]$Repo = "ap1529921128-design/voice-ime-rust-2.0.1",
    [string]$Tag = "v2.0.1",
    [string]$Title = "Voice IME Rust 2.0.1",
    [string]$AssetsManifest = "D:\voice-ime-build-release\voice-ime-release-assets-2.0.1.json",
    [string]$NotesPath = "docs\release-notes-2.0.1.md",
    [string]$Token = "",
    [switch]$PromptForToken,
    [switch]$ValidateOnly,
    [switch]$Draft,
    [int]$UploadRetryCount = 3
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

function Get-ResponseStatusCode {
    param([Parameter(Mandatory = $true)]$ErrorRecord)
    if ($ErrorRecord.Exception.Response) {
        return $ErrorRecord.Exception.Response.StatusCode.value__
    }
    return $null
}

function Get-ResponseBodyMessage {
    param([Parameter(Mandatory = $true)]$ErrorRecord)
    if ($ErrorRecord.ErrorDetails -and -not [string]::IsNullOrWhiteSpace($ErrorRecord.ErrorDetails.Message)) {
        try {
            $json = $ErrorRecord.ErrorDetails.Message | ConvertFrom-Json -ErrorAction Stop
            $parts = @()
            if ($json.message) {
                $parts += [string]$json.message
            }
            foreach ($errorItem in @($json.errors)) {
                $detail = @(
                    $errorItem.resource,
                    $errorItem.field,
                    $errorItem.code,
                    $errorItem.message
                ) | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) }
                if ($detail.Count -gt 0) {
                    $parts += ($detail -join "/")
                }
            }
            if ($parts.Count -gt 0) {
                return ($parts -join " | ")
            }
        }
        catch {
            return [string]$ErrorRecord.ErrorDetails.Message
        }
    }
    $response = $ErrorRecord.Exception.Response
    if (-not $response) {
        return ""
    }
    try {
        $stream = $response.GetResponseStream()
        if (-not $stream) {
            return ""
        }
        $reader = [System.IO.StreamReader]::new($stream)
        $body = $reader.ReadToEnd()
        if ([string]::IsNullOrWhiteSpace($body)) {
            return ""
        }
        $json = $body | ConvertFrom-Json -ErrorAction SilentlyContinue
        if ($json -and $json.message) {
            return [string]$json.message
        }
        return $body
    }
    catch {
        return ""
    }
}

function Format-GitHubAuthError {
    param(
        [Parameter(Mandatory = $true)]$ErrorRecord,
        [Parameter(Mandatory = $true)][string]$Repo
    )
    $status = Get-ResponseStatusCode -ErrorRecord $ErrorRecord
    $message = Get-ResponseBodyMessage -ErrorRecord $ErrorRecord
    if ($status -eq 401) {
        return @(
            "GitHub rejected the token with 401 Unauthorized.",
            "Check that you pasted the token value itself, not an SSH key, key fingerprint, token name, or token id.",
            "A valid fine-grained token usually starts with github_pat_; a classic token usually starts with ghp_.",
            "For fine-grained tokens, select repository '$Repo' and grant Contents: Read and write.",
            "If the token was just created, copy the one-time token value again or create a new token."
        ) -join " "
    }
    if ($status -eq 403) {
        return @(
            "GitHub accepted the token but refused this operation with 403 Forbidden.",
            "Grant Contents: Read and write for repository '$Repo', or use a classic token with repo scope."
        ) -join " "
    }
    if ($status -eq 404) {
        return "GitHub returned 404. Check repository '$Repo', tag/release visibility, and whether the token has access to this repository."
    }
    if ($status -eq 422) {
        if (-not [string]::IsNullOrWhiteSpace($message)) {
            return "GitHub rejected the release request with 422 Validation Failed: $message"
        }
        return "GitHub rejected the release request with 422 Validation Failed. Check whether the release/tag already exists or whether target branch 'main' exists."
    }
    if (-not [string]::IsNullOrWhiteSpace($message)) {
        return "GitHub API error ${status}: $message"
    }
    return $ErrorRecord.Exception.Message
}

function Get-AssetContentType {
    param([Parameter(Mandatory = $true)][string]$Path)
    switch -Regex ([System.IO.Path]::GetExtension($Path).ToLowerInvariant()) {
        '^\.zip$' { return "application/zip" }
        '^\.json$' { return "application/json; charset=utf-8" }
        '^\.md$' { return "text/markdown; charset=utf-8" }
        default { return "application/octet-stream" }
    }
}

function Invoke-WithRetry {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$Label,
        [int]$RetryCount = 3
    )
    $attempt = 0
    while ($true) {
        $attempt += 1
        try {
            return & $Action
        }
        catch {
            if ($attempt -ge [Math]::Max(1, $RetryCount)) {
                throw
            }
            $delay = [Math]::Min(30, 2 * $attempt)
            Write-Warning "$Label failed on attempt $attempt; retrying in $delay seconds. $($_.Exception.Message)"
            Start-Sleep -Seconds $delay
        }
    }
}

function Resolve-GitHubCli {
    $command = Get-Command gh -ErrorAction SilentlyContinue
    if ($command -and $command.Source) {
        return $command.Source
    }

    $candidateRoots = @(
        (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"),
        (Join-Path $env:ProgramFiles "GitHub CLI"),
        (Join-Path ${env:ProgramFiles(x86)} "GitHub CLI")
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_) }

    foreach ($root in $candidateRoots) {
        $candidate = Get-ChildItem -LiteralPath $root -Recurse -Filter "gh.exe" -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "\\GitHub\.cli|\\GitHub CLI\\" } |
            Sort-Object FullName |
            Select-Object -First 1
        if ($candidate) {
            return $candidate.FullName
        }
    }
    return $null
}

function Read-GitHubTokenFromConsole {
    Write-Host ""
    Write-Host "GitHub Release asset upload needs GitHub API authorization."
    Write-Host "Paste a token in this local PowerShell window only. It will not be saved."
    Write-Host "Required permission: repo scope, or fine-grained Contents Read and write for this repository."
    $secure = Read-Host "GH_TOKEN (leave empty to cancel)" -AsSecureString
    if ($secure.Length -eq 0) {
        return ""
    }

    $bstr = [System.IntPtr]::Zero
    try {
        $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
    }
    finally {
        if ($bstr -ne [System.IntPtr]::Zero) {
            [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
        }
    }
}

function Invoke-NativeExitCode {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    try {
        & $FilePath @Arguments *> $null
        return $LASTEXITCODE
    }
    catch {
        if ($null -ne $LASTEXITCODE) {
            return $LASTEXITCODE
        }
        return 1
    }
}

function Publish-WithGitHubCli {
    param(
        [Parameter(Mandatory = $true)][string]$GitHubCli,
        [Parameter(Mandatory = $true)][string]$Repo,
        [Parameter(Mandatory = $true)][string]$Tag,
        [Parameter(Mandatory = $true)][string]$Title,
        [Parameter(Mandatory = $true)][string]$NotesPath,
        [Parameter(Mandatory = $true)]$Assets,
        [string]$Token = "",
        [switch]$Draft
    )
    $previousToken = $env:GH_TOKEN
    $hadPreviousToken = Test-Path Env:\GH_TOKEN
    if (-not [string]::IsNullOrWhiteSpace($Token)) {
        $env:GH_TOKEN = $Token
    }
    try {
        $releaseViewExitCode = Invoke-NativeExitCode -FilePath $GitHubCli -Arguments @("release", "view", $Tag, "--repo", $Repo)
        if ($releaseViewExitCode -eq 0) {
            & $GitHubCli release edit $Tag --repo $Repo --title $Title --notes-file $NotesPath
            if ($LASTEXITCODE -ne 0) {
                throw "gh release edit failed"
            }
            & $GitHubCli release upload $Tag @($Assets | ForEach-Object { $_.path }) --repo $Repo --clobber
            if ($LASTEXITCODE -ne 0) {
                throw "gh release upload failed"
            }
        }
        else {
            $ghArgs = @("release", "create", $Tag) + @($Assets | ForEach-Object { $_.path }) + @("--repo", $Repo, "--title", $Title, "--notes-file", $NotesPath, "--verify-tag")
            if ($Draft) {
                $ghArgs += "--draft"
            }
            & $GitHubCli @ghArgs
            if ($LASTEXITCODE -ne 0) {
                throw "gh release create failed"
            }
        }
        Write-Host "GitHub release published with gh: https://github.com/$Repo/releases/tag/$Tag"
    }
    finally {
        if ($hadPreviousToken) {
            $env:GH_TOKEN = $previousToken
        }
        else {
            Remove-Item Env:\GH_TOKEN -ErrorAction SilentlyContinue
        }
    }
}

$manifest = Get-Content -LiteralPath $AssetsManifest -Raw | ConvertFrom-Json
$assets = @()
foreach ($asset in @($manifest.assets)) {
    if (-not (Test-Path -LiteralPath $asset.path -PathType Leaf)) {
        throw "Asset missing: $($asset.path)"
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$asset.sha256)) {
        $actualHash = (Get-FileHash -LiteralPath ([string]$asset.path) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne ([string]$asset.sha256).ToLowerInvariant()) {
            throw "Asset hash mismatch for $($asset.name): expected $($asset.sha256), got $actualHash"
        }
    }
    $assets += $asset
}
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

Write-Host "Release upload set:"
foreach ($asset in $assets) {
    $item = Get-Item -LiteralPath ([string]$asset.path)
    Write-Host (" - {0} {1} MB" -f $item.Name, [Math]::Round($item.Length / 1MB, 1))
}

if ($ValidateOnly) {
    Write-Host "Validation passed. Assets are ready for https://github.com/$Repo/releases/tag/$Tag"
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

$gh = Resolve-GitHubCli
$canUseGitHubCli = $false
if ($gh -and [string]::IsNullOrWhiteSpace($Token)) {
    $authExitCode = Invoke-NativeExitCode -FilePath $gh -Arguments @("auth", "status", "--hostname", "github.com")
    if ($authExitCode -ne 0) {
        if ($PromptForToken) {
            Write-Warning "GitHub CLI is installed at $gh but is not authenticated. Falling back to local token prompt."
        }
        else {
            throw "GitHub CLI is installed at $gh but is not authenticated. Run `gh auth login --hostname github.com --git-protocol ssh --scopes repo`, set `$env:GH_TOKEN, or rerun this script with -PromptForToken."
        }
    }
    else {
        $canUseGitHubCli = $true
    }
}
if ($canUseGitHubCli -and [string]::IsNullOrWhiteSpace($Token)) {
    Publish-WithGitHubCli -GitHubCli $gh -Repo $Repo -Tag $Tag -Title $Title -NotesPath $NotesPath -Assets $assets -Draft:$Draft
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Token) -and $PromptForToken) {
    $Token = Read-GitHubTokenFromConsole
}

function Invoke-GitHubRest {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)]$Headers,
        [object]$Body = $null,
        [string]$ContentType = ""
    )
    try {
        $params = @{
            Method  = $Method
            Uri     = $Uri
            Headers = $Headers
        }
        if ($null -ne $Body) {
            $params.Body = $Body
        }
        if (-not [string]::IsNullOrWhiteSpace($ContentType)) {
            $params.ContentType = $ContentType
        }
        return Invoke-RestMethod @params
    }
    catch {
        throw (Format-GitHubAuthError -ErrorRecord $_ -Repo $Repo)
    }
}
$Token = $Token.Trim().Trim('"').Trim("'")

if ([string]::IsNullOrWhiteSpace($Token)) {
    throw "GitHub CLI is not installed/authenticated and no GH_TOKEN/GITHUB_TOKEN was provided. Install and authenticate gh, set `$env:GH_TOKEN, or rerun with -PromptForToken: powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\publish-github-release.ps1 -PromptForToken"
}

if ($gh) {
    Publish-WithGitHubCli -GitHubCli $gh -Repo $Repo -Tag $Tag -Title $Title -NotesPath $NotesPath -Assets $assets -Token $Token -Draft:$Draft
    exit 0
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
    $status = Get-ResponseStatusCode -ErrorRecord $_
    if ($status -in 401, 403) {
        throw (Format-GitHubAuthError -ErrorRecord $_ -Repo $Repo)
    }
    if ($status -ne 404) {
        throw
    }
    $body = @{
        tag_name   = $Tag
        target_commitish = "main"
        name       = $Title
        body       = $notes
        draft      = [bool]$Draft
        prerelease = $false
    } | ConvertTo-Json -Depth 4
    $release = Invoke-GitHubRest -Method Post -Uri "$apiBase/releases" -Headers $headers -Body $body -ContentType "application/json"
}

$existingAssets = Invoke-GitHubRest -Method Get -Uri $release.assets_url -Headers $headers
$uploadBase = $release.upload_url -replace "\{\?name,label\}$", ""

foreach ($asset in $assets) {
    $existing = @($existingAssets | Where-Object { $_.name -eq $asset.name } | Select-Object -First 1)
    if ($existing.Count -gt 0) {
        Invoke-GitHubRest -Method Delete -Uri "$apiBase/releases/assets/$($existing[0].id)" -Headers $headers | Out-Null
    }
    $encodedName = [System.Uri]::EscapeDataString([string]$asset.name)
    Invoke-WithRetry -RetryCount $UploadRetryCount -Label "Upload $($asset.name)" -Action {
        Invoke-RestMethod `
            -Method Post `
            -Uri "$uploadBase?name=$encodedName" `
            -Headers $headers `
            -ContentType (Get-AssetContentType -Path ([string]$asset.path)) `
            -InFile ([string]$asset.path) | Out-Null
    } | Out-Null
    Write-Host "Uploaded: $($asset.name)"
}

Write-Host "GitHub release published: https://github.com/$Repo/releases/tag/$Tag"
