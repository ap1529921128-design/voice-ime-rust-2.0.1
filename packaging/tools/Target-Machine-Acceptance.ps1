param(
    [string]$SamplesDir = "",
    [switch]$SkipDoctor,
    [switch]$SkipAsrTemplate,
    [switch]$SkipNotepad,
    [switch]$SkipBrowser,
    [switch]$SkipTranslation,
    [switch]$RunForeground,
    [string]$ExpectedProcess = "",
    [switch]$NoOpen
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$LogsDir = Join-Path $AppDir ".voice_ime\logs"
$ReportPath = Join-Path $LogsDir ("target-machine-acceptance-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}

New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null

if ([string]::IsNullOrWhiteSpace($SamplesDir)) {
    $SamplesDir = Join-Path $AppDir "benchmarks\asr"
}

$script:Rows = [System.Collections.Generic.List[object]]::new()

function Add-Row {
    param(
        [string]$Name,
        [string]$Status,
        [string]$Detail
    )
    $script:Rows.Add([pscustomobject]@{
        name   = $Name
        status = $Status
        detail = $Detail
    }) | Out-Null
    Write-Host ("[{0}] {1} {2}" -f $Status, $Name, $Detail)
}

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Body
    )
    try {
        $detail = & $Body
        if ([string]::IsNullOrWhiteSpace([string]$detail)) {
            $detail = "ok"
        }
        Add-Row -Name $Name -Status "PASS" -Detail ([string]$detail)
    }
    catch {
        Add-Row -Name $Name -Status "FAIL" -Detail $_.Exception.Message
    }
}

function Invoke-ToolScript {
    param(
        [string]$ScriptName,
        [string[]]$Arguments = @()
    )
    $scriptPath = Join-Path $PSScriptRoot $ScriptName
    if (-not (Test-Path -LiteralPath $scriptPath -PathType Leaf)) {
        throw "tool script missing: $scriptPath"
    }
    & powershell -NoProfile -ExecutionPolicy Bypass -File $scriptPath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$ScriptName exited with code $LASTEXITCODE"
    }
    return $ScriptName
}

if ($SkipDoctor) {
    Add-Row -Name "Doctor" -Status "SKIP" -Detail "skipped by flag"
}
else {
    Invoke-Step -Name "Doctor" -Body {
        $process = Start-Process -FilePath $Exe -ArgumentList @("--doctor") -WorkingDirectory $AppDir -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "VoiceIME.exe --doctor exited with code $($process.ExitCode)"
        }
        $report = Get-ChildItem -LiteralPath $LogsDir -Filter "doctor-*.txt" -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($report) {
            return $report.FullName
        }
        return "doctor finished"
    }
}

if ($SkipAsrTemplate) {
    Add-Row -Name "ASR Template" -Status "SKIP" -Detail "skipped by flag"
}
else {
    Invoke-Step -Name "ASR Template" -Body {
        Invoke-ToolScript -ScriptName "ASR-Benchmark.ps1" -Arguments @("-SamplesDir", $SamplesDir, "-TemplateOnly", "-NoOpen") | Out-Null
        return $SamplesDir
    }
}

if ($SkipNotepad) {
    Add-Row -Name "Notepad Paste" -Status "SKIP" -Detail "skipped by flag"
}
else {
    Invoke-Step -Name "Notepad Paste" -Body {
        Invoke-ToolScript -ScriptName "Notepad-Input-Acceptance.ps1" | Out-Null
        $report = Get-ChildItem -LiteralPath $LogsDir -Filter "notepad-acceptance-*.txt" -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($report) { return $report.FullName }
        return "notepad acceptance finished"
    }
}

if ($SkipBrowser) {
    Add-Row -Name "Browser Paste" -Status "SKIP" -Detail "skipped by flag"
}
else {
    Invoke-Step -Name "Browser Paste" -Body {
        Invoke-ToolScript -ScriptName "Browser-Input-Acceptance.ps1" | Out-Null
        $report = Get-ChildItem -LiteralPath $LogsDir -Filter "browser-acceptance-*.txt" -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($report) { return $report.FullName }
        return "browser acceptance finished"
    }
}

if ($SkipTranslation) {
    Add-Row -Name "Translation" -Status "SKIP" -Detail "skipped by flag"
}
else {
    Invoke-Step -Name "Translation" -Body {
        Invoke-ToolScript -ScriptName "Translation-Acceptance.ps1" | Out-Null
        return "translation acceptance finished"
    }
}

if ($RunForeground) {
    Invoke-Step -Name "Foreground App" -Body {
        $args = @()
        if (-not [string]::IsNullOrWhiteSpace($ExpectedProcess)) {
            $args += @("-ExpectedProcess", $ExpectedProcess)
        }
        Invoke-ToolScript -ScriptName "Foreground-Input-Acceptance.ps1" -Arguments $args | Out-Null
        $report = Get-ChildItem -LiteralPath $LogsDir -Filter "foreground-acceptance-*.txt" -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($report) { return $report.FullName }
        return "foreground acceptance finished"
    }
}
else {
    Add-Row -Name "Foreground App" -Status "SKIP" -Detail "use -RunForeground -ExpectedProcess <process.exe>"
}

$failed = @($script:Rows | Where-Object { $_.status -eq "FAIL" })
$passed = @($script:Rows | Where-Object { $_.status -eq "PASS" })
$skipped = @($script:Rows | Where-Object { $_.status -eq "SKIP" })
$overall = ($failed.Count -eq 0)

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("Voice IME Target Machine Acceptance")
$lines.Add("created_at=$((Get-Date).ToString("o"))")
$lines.Add("passed=$overall")
$lines.Add("app_dir=$AppDir")
$lines.Add("logs_dir=$LogsDir")
$lines.Add("samples_dir=$SamplesDir")
$lines.Add("pass_count=$($passed.Count)")
$lines.Add("fail_count=$($failed.Count)")
$lines.Add("skip_count=$($skipped.Count)")
$lines.Add("")
foreach ($row in $script:Rows) {
    $lines.Add("$($row.status)`t$($row.name)`t$($row.detail)")
}

Set-Content -LiteralPath $ReportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
Write-Host ""
Write-Host ($lines -join [Environment]::NewLine)
Write-Host "Report: $ReportPath"

if (-not $NoOpen) {
    Start-Process -FilePath "explorer.exe" -ArgumentList $LogsDir | Out-Null
}

if (-not $overall) {
    exit 2
}
