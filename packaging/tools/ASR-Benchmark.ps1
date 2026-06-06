param(
    [string]$SamplesDir = "",
    [string[]]$Profiles = @(),
    [switch]$TemplateOnly,
    [switch]$NoOpen
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$RuntimeDir = if (-not [string]::IsNullOrWhiteSpace($env:VOICE_IME_APP_DIR)) {
    [System.IO.Path]::GetFullPath($env:VOICE_IME_APP_DIR)
}
else {
    Join-Path $AppDir ".voice_ime"
}
$LogsDir = Join-Path $RuntimeDir "logs"

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}

if ([string]::IsNullOrWhiteSpace($SamplesDir)) {
    $SamplesDir = Join-Path $AppDir "benchmarks\asr"
}

$SamplesDir = [System.IO.Path]::GetFullPath($SamplesDir)
New-Item -ItemType Directory -Path $SamplesDir -Force | Out-Null
New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null

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

function ConvertTo-OptionalDouble {
    param([object]$Value)

    $text = ([string]$Value).Trim()
    if ([string]::IsNullOrWhiteSpace($text)) {
        return $null
    }
    $parsed = 0.0
    $culture = [System.Globalization.CultureInfo]::InvariantCulture
    if ([double]::TryParse($text, [System.Globalization.NumberStyles]::Float, $culture, [ref]$parsed)) {
        return $parsed
    }
    return $null
}

function Get-AverageText {
    param(
        [object[]]$Rows,
        [string]$Property,
        [int]$Decimals
    )

    $values = [System.Collections.Generic.List[double]]::new()
    foreach ($row in $Rows) {
        $value = ConvertTo-OptionalDouble $row.$Property
        if ($null -ne $value) {
            $values.Add([double]$value) | Out-Null
        }
    }
    if ($values.Count -eq 0) {
        return "na"
    }
    $average = ($values | Measure-Object -Average).Average
    return $average.ToString(("F" + $Decimals), [System.Globalization.CultureInfo]::InvariantCulture)
}

function Get-UniqueText {
    param(
        [object[]]$Rows,
        [string]$Property
    )

    $values = @($Rows |
        ForEach-Object { [string]$_.$Property } |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        Select-Object -Unique)
    if ($values.Count -eq 0) {
        return "na"
    }
    return ($values -join ",")
}

function New-AsrBenchmarkSummary {
    param(
        [System.IO.FileInfo[]]$Reports,
        [string]$SamplesDir
    )

    if (-not $Reports -or $Reports.Count -eq 0) {
        return ""
    }

    $rows = [System.Collections.Generic.List[object]]::new()
    foreach ($report in $Reports) {
        $csvRows = @(Import-Csv -LiteralPath $report.FullName -Encoding UTF8)
        foreach ($row in $csvRows) {
            $row | Add-Member -NotePropertyName "__report" -NotePropertyValue $report.FullName -Force
            $rows.Add($row) | Out-Null
        }
    }

    $summaryPath = Join-Path $LogsDir ("asr-benchmark-summary-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("Voice IME ASR Benchmark Summary")
    $lines.Add("created_at=$((Get-Date).ToString("o"))")
    $lines.Add("samples_dir=$SamplesDir")
    $lines.Add("logs_dir=$LogsDir")
    $lines.Add("reports_count=$($Reports.Count)")
    $lines.Add("rows_count=$($rows.Count)")
    $lines.Add("")

    $groups = $rows | Group-Object -Property profile | Sort-Object Name
    foreach ($group in $groups) {
        $groupRows = @($group.Group)
        $profile = if ([string]::IsNullOrWhiteSpace($group.Name)) { "unknown" } else { $group.Name }
        $errors = @($groupRows | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_.error) })
        $okCount = $groupRows.Count - $errors.Count
        $line = "PROFILE`t{0}`trows={1}`tok={2}`terrors={3}`tavg_seconds={4}`tavg_rtf={5}`tavg_cer={6}`tavg_accuracy={7}`tbackend={8}`tmodel={9}" -f @(
            $profile,
            $groupRows.Count,
            $okCount,
            $errors.Count,
            (Get-AverageText -Rows $groupRows -Property "transcribe_seconds" -Decimals 3),
            (Get-AverageText -Rows $groupRows -Property "rtf" -Decimals 3),
            (Get-AverageText -Rows $groupRows -Property "cer" -Decimals 4),
            (Get-AverageText -Rows $groupRows -Property "accuracy" -Decimals 4),
            (Get-UniqueText -Rows $groupRows -Property "backend"),
            (Get-UniqueText -Rows $groupRows -Property "model")
        )
        $lines.Add($line)
    }

    $lines.Add("")
    foreach ($report in $Reports) {
        $lines.Add("REPORT`t$($report.FullName)")
    }

    Set-Content -LiteralPath $summaryPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
    return $summaryPath
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

$benchmarkStarted = Get-Date

$profileList = [System.Collections.Generic.List[string]]::new()
foreach ($profileValue in $Profiles) {
    foreach ($profile in ([string]$profileValue -split ",")) {
        $profile = $profile.Trim()
        if ($profile.Length -gt 0) {
            $profileList.Add($profile) | Out-Null
        }
    }
}

if ($profileList.Count -eq 0) {
    Invoke-VoiceImeCli -Arguments @("--benchmark-asr", $SamplesDir)
}
else {
    foreach ($profile in $profileList) {
        if ($profile -notin @("fast", "balanced", "fallback", "accurate")) {
            throw "Unknown ASR profile '$profile'. Expected fast, balanced, fallback, or accurate."
        }
        Invoke-VoiceImeCli -Arguments @("--benchmark-asr-profile", $profile, $SamplesDir)
    }
}

$reports = @(Get-ChildItem -LiteralPath $LogsDir -Filter "asr-benchmark-*.csv" -File -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -ge $benchmarkStarted.AddSeconds(-2) } |
    Sort-Object LastWriteTime)

if ($reports.Count -eq 0) {
    $reports = @(Get-ChildItem -LiteralPath $LogsDir -Filter "asr-benchmark-*.csv" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1)
}

$latestReport = $reports |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if ($latestReport) {
    Write-Host "Latest report: $($latestReport.FullName)"
    $summaryPath = New-AsrBenchmarkSummary -Reports $reports -SamplesDir $SamplesDir
    if (-not [string]::IsNullOrWhiteSpace($summaryPath)) {
        Write-Host "Summary: $summaryPath"
    }
}
else {
    Write-Host "Benchmark finished, but no CSV report was found under $LogsDir"
}

if (-not $NoOpen) {
    Start-Process -FilePath "explorer.exe" -ArgumentList $LogsDir | Out-Null
}
