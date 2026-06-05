param(
    [string]$CoreReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core",
    [string]$ModelPackZip = "D:\voice-ime-build-release\voice-ime-model-pack-asr-fallback-whisper-tiny-int8.zip",
    [switch]$KeepWorkDir
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $CoreReleaseRoot -PathType Container)) {
    throw "Core release root missing: $CoreReleaseRoot"
}
if (-not (Test-Path -LiteralPath $ModelPackZip -PathType Leaf)) {
    throw "Model pack zip missing: $ModelPackZip"
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Get-ZipMetadata {
    param([Parameter(Mandatory = $true)][string]$ZipPath)
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $entry = $zip.Entries | Where-Object { $_.FullName -eq "MODEL_PACK.json" } | Select-Object -First 1
        if (-not $entry) {
            throw "MODEL_PACK.json missing from $ZipPath"
        }
        $reader = [System.IO.StreamReader]::new($entry.Open())
        try {
            return $reader.ReadToEnd() | ConvertFrom-Json
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $zip.Dispose()
    }
}

function ConvertTo-InstalledModelPath {
    param(
        [Parameter(Mandatory = $true)][string]$ModelsDir,
        [Parameter(Mandatory = $true)][string]$PackPath
    )
    $normalized = $PackPath -replace '\\', '/'
    if ($normalized.StartsWith("app/models/")) {
        $relative = $normalized.Substring("app/models/".Length)
    }
    elseif ($normalized.StartsWith("models/")) {
        $relative = $normalized.Substring("models/".Length)
    }
    elseif ($normalized -eq "MODEL_PACK.txt" -or $normalized -eq "MODEL_PACK.json") {
        $relative = $normalized
    }
    else {
        return $null
    }
    if ($relative.Contains("..") -or $relative -match '^[A-Za-z]:') {
        throw "Unsafe model pack path in metadata: $PackPath"
    }
    return Join-Path $ModelsDir ($relative -replace '/', [System.IO.Path]::DirectorySeparatorChar)
}

function Assert-InstalledFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][int64]$Bytes,
        [Parameter(Mandatory = $true)][string]$Sha256
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Installed file missing: $Path"
    }
    $item = Get-Item -LiteralPath $Path
    if ($item.Length -ne $Bytes) {
        throw "Installed file size mismatch: $Path expected=$Bytes actual=$($item.Length)"
    }
    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256.ToLowerInvariant()) {
        throw "Installed file SHA-256 mismatch: $Path"
    }
}

$workRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-model-import-" + [guid]::NewGuid().ToString("N"))
$testRoot = Join-Path $workRoot "core"
$previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")

try {
    New-Item -ItemType Directory -Path $workRoot -Force | Out-Null
    Copy-Item -LiteralPath $CoreReleaseRoot -Destination $testRoot -Recurse -Force
    $appDir = Join-Path $testRoot "app"
    $exe = Join-Path $appDir "VoiceIME.exe"
    $modelsDir = Join-Path $appDir "models"
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "Copied core package is missing VoiceIME.exe: $exe"
    }
    $unexpectedBefore = @(Get-ChildItem -LiteralPath $modelsDir -Force | Where-Object { $_.Name -notin @("MODELS.json", "MODELS.md") })
    if ($unexpectedBefore.Count -gt 0) {
        throw "Copied core models directory is not clean before import: $($unexpectedBefore.Name -join ', ')"
    }

    $metadata = Get-ZipMetadata -ZipPath $ModelPackZip
    $env:VOICE_IME_APP_DIR = Join-Path $workRoot ".voice_ime"
    $process = Start-Process -FilePath $exe `
        -ArgumentList @("--install-model-pack", $ModelPackZip) `
        -WorkingDirectory $appDir `
        -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $workRoot "install-stdout.json") `
        -RedirectStandardError (Join-Path $workRoot "install-stderr.txt") `
        -Wait `
        -PassThru
    if ($process.ExitCode -ne 0) {
        $stderr = Get-Content -LiteralPath (Join-Path $workRoot "install-stderr.txt") -Raw -ErrorAction SilentlyContinue
        throw "model pack install exited with code $($process.ExitCode): $stderr"
    }
    $stdout = Get-Content -LiteralPath (Join-Path $workRoot "install-stdout.json") -Raw
    $report = $stdout | ConvertFrom-Json
    if ([int]$report.files_written -le 0) {
        throw "model pack install wrote no files"
    }
    if ([int]$report.checksum_verified -le 0) {
        throw "model pack install did not verify metadata checksums"
    }

    $verified = 0
    foreach ($file in $metadata.files) {
        $installed = ConvertTo-InstalledModelPath -ModelsDir $modelsDir -PackPath ([string]$file.path)
        if (-not $installed) {
            continue
        }
        Assert-InstalledFile -Path $installed -Bytes ([int64]$file.bytes) -Sha256 ([string]$file.sha256)
        $verified += 1
    }
    if ($verified -eq 0) {
        throw "No installed files were verified from MODEL_PACK.json"
    }

    $lines = @(
        "Voice IME Model Pack Import Acceptance",
        "created_at=$((Get-Date).ToString("o"))",
        "passed=True",
        "core_source=$CoreReleaseRoot",
        "model_pack=$ModelPackZip",
        "work_dir=$workRoot",
        "files_written=$($report.files_written)",
        "checksum_verified=$($report.checksum_verified)",
        "verified_files=$verified",
        "bytes_written=$($report.bytes_written)"
    )
    Write-Host ($lines -join [Environment]::NewLine)
}
finally {
    if ([string]::IsNullOrEmpty($previousAppDir)) {
        Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:VOICE_IME_APP_DIR = $previousAppDir
    }
    if (-not $KeepWorkDir) {
        Remove-Item -LiteralPath $workRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
