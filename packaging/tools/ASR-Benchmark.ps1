param(
    [string]$SamplesDir = "",
    [string[]]$Profiles = @(),
    [switch]$TemplateOnly,
    [switch]$NoOpen
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$LogsDir = Join-Path $AppDir ".voice_ime\logs"

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}

if ([string]::IsNullOrWhiteSpace($SamplesDir)) {
    $SamplesDir = Join-Path $AppDir "benchmarks\asr"
}

$SamplesDir = [System.IO.Path]::GetFullPath($SamplesDir)
New-Item -ItemType Directory -Path $SamplesDir -Force | Out-Null

function Invoke-VoiceImeCli {
    param([string[]]$Arguments)
    $process = Start-Process -FilePath $Exe `
        -ArgumentList $Arguments `
        -WorkingDirectory $AppDir `
        -WindowStyle Hidden `
        -Wait `
        -PassThru
    if ($process.ExitCode -ne 0) {
        throw "VoiceIME.exe $($Arguments -join ' ') exited with code $($process.ExitCode)"
    }
}

Invoke-VoiceImeCli -Arguments @("--write-asr-benchmark-template", $SamplesDir)

$wavFiles = @(Get-ChildItem -LiteralPath $SamplesDir -Filter "*.wav" -File -ErrorAction SilentlyContinue | Sort-Object Name)
Write-Host "ASR benchmark folder: $SamplesDir"
Write-Host "Reference templates are ready. Record 001.wav through 010.wav next to the matching .txt files."

if ($TemplateOnly -or $wavFiles.Count -eq 0) {
    if ($wavFiles.Count -eq 0) {
        Write-Host "No wav files found yet; template setup only."
    }
    if (-not $NoOpen) {
        Start-Process -FilePath "explorer.exe" -ArgumentList $SamplesDir | Out-Null
    }
    exit 0
}

if ($Profiles.Count -eq 0) {
    Invoke-VoiceImeCli -Arguments @("--benchmark-asr", $SamplesDir)
}
else {
    foreach ($profile in $Profiles) {
        $profile = $profile.Trim()
        if ($profile.Length -eq 0) {
            continue
        }
        if ($profile -notin @("fast", "balanced", "fallback", "accurate")) {
            throw "Unknown ASR profile '$profile'. Expected fast, balanced, fallback, or accurate."
        }
        Invoke-VoiceImeCli -Arguments @("--benchmark-asr-profile", $profile, $SamplesDir)
    }
}

$latestReport = Get-ChildItem -LiteralPath $LogsDir -Filter "asr-benchmark-*.csv" -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if ($latestReport) {
    Write-Host "Latest report: $($latestReport.FullName)"
}
else {
    Write-Host "Benchmark finished, but no CSV report was found under $LogsDir"
}

if (-not $NoOpen) {
    Start-Process -FilePath "explorer.exe" -ArgumentList $LogsDir | Out-Null
}
