param(
    [switch]$KeepAppDir
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$MockTranslator = Join-Path $PSScriptRoot "Mock-External-Translate.ps1"

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}
if (-not (Test-Path -LiteralPath $MockTranslator -PathType Leaf)) {
    throw "Mock translator not found: $MockTranslator"
}

function Quote-JsonString {
    param([string]$Value)
    return ($Value | ConvertTo-Json -Compress)
}

function Quote-ProcessArgument {
    param([string]$Value)
    if ($null -eq $Value) {
        $Value = ""
    }
    if ($Value.Length -eq 0) {
        return '""'
    }
    if ($Value -notmatch '[\s"]') {
        return $Value
    }

    $quoted = '"'
    $slashes = 0
    foreach ($char in $Value.ToCharArray()) {
        if ($char -eq '\') {
            $slashes += 1
            continue
        }
        if ($char -eq '"') {
            $quoted += ('\' * (($slashes * 2) + 1)) + '"'
            $slashes = 0
            continue
        }
        if ($slashes -gt 0) {
            $quoted += '\' * $slashes
            $slashes = 0
        }
        $quoted += $char
    }
    if ($slashes -gt 0) {
        $quoted += '\' * ($slashes * 2)
    }
    $quoted += '"'
    return $quoted
}

function Join-Codepoints {
    param([int[]]$Codepoints)
    return (($Codepoints | ForEach-Object { [char]$_ }) -join "")
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Value
    )
    $encoding = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($Path, $Value, $encoding)
}

$acceptanceRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-translation-acceptance-" + [guid]::NewGuid().ToString("N"))
$logsDir = Join-Path $acceptanceRoot "logs"
$samplePath = Join-Path $acceptanceRoot "translation-samples.tsv"
$reportPath = Join-Path $acceptanceRoot ("translation-acceptance-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")
$previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")

try {
    New-Item -ItemType Directory -Path $logsDir -Force | Out-Null
    $externalCommand = "powershell -NoProfile -ExecutionPolicy Bypass -File $(Quote-ProcessArgument $MockTranslator)"
    $config = @"
{
  "asr": {},
  "input": {},
  "translation": {
    "engine": "external",
    "profile": "fast",
    "external_command": "",
    "timeout_seconds": 3,
    "models": {
      "fast_command": $(Quote-JsonString $externalCommand),
      "balanced_command": "",
      "accurate_command": ""
    }
  }
}
"@
    Write-Utf8NoBom -Path (Join-Path $acceptanceRoot "config.json") -Value $config
    $zhSource = Join-Codepoints @(0x7ffb, 0x8bd1, 0x7ed3, 0x679c, 0xff1a, 0x975e, 0x6d32, 0x4e4b, 0x661f, 0x548c, 0x6d77, 0x6d0b, 0x4e4b, 0x6cea)
    $zhHint = Join-Codepoints @(0x975e, 0x6d32, 0x4e4b, 0x661f)
    $samples = @(
        "en`tsettings page local service`tLocal",
        "ja`tlocal first, no default cloud upload`t",
        "zh`t$zhSource`t$zhHint"
    ) -join [Environment]::NewLine
    Write-Utf8NoBom -Path $samplePath -Value $samples

    $env:VOICE_IME_APP_DIR = $acceptanceRoot
    $process = Start-Process -FilePath $Exe `
        -ArgumentList @("--benchmark-translation", $samplePath) `
        -WorkingDirectory $AppDir `
        -WindowStyle Hidden `
        -Wait `
        -PassThru
    if ($process.ExitCode -ne 0) {
        throw "translation benchmark exited with code $($process.ExitCode)"
    }

    $benchmark = Get-ChildItem -LiteralPath $logsDir -Filter "translation-benchmark-*.csv" -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $benchmark) {
        throw "translation benchmark CSV missing under $logsDir"
    }
    $rows = @(Import-Csv -LiteralPath $benchmark.FullName)
    if ($rows.Count -ne 3) {
        throw "expected 3 benchmark rows, got $($rows.Count)"
    }
    $wrongModel = @($rows | Where-Object { $_.model -ne "mt/fast" })
    if ($wrongModel.Count -gt 0) {
        throw "translation acceptance did not use mt/fast profile: $($wrongModel | ConvertTo-Json -Compress)"
    }
    $failed = @($rows | Where-Object {
        $_.error -or
        $_.language_match -ne "true" -or
        ($_.expected_hint -and $_.expected_hint_match -ne "true")
    })
    if ($failed.Count -gt 0) {
        throw "translation acceptance failed rows: $($failed | ConvertTo-Json -Compress)"
    }

    $lines = @(
        "Voice IME Translation Acceptance",
        "created_at=$((Get-Date).ToString("o"))",
        "passed=True",
        "app_dir=$acceptanceRoot",
        "benchmark=$($benchmark.FullName)",
        "rows=$($rows.Count)",
        "translator=$MockTranslator"
    )
    Set-Content -LiteralPath $reportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
    Write-Host ($lines -join [Environment]::NewLine)
}
finally {
    if ([string]::IsNullOrEmpty($previousAppDir)) {
        Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:VOICE_IME_APP_DIR = $previousAppDir
    }
    if (-not $KeepAppDir) {
        Remove-Item -LiteralPath $acceptanceRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
