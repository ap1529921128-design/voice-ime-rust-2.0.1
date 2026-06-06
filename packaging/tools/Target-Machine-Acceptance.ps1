param(
    [string]$SamplesDir = "",
    [switch]$SkipDoctor,
    [switch]$SkipModelRoot,
    [switch]$SkipAsrTemplate,
    [switch]$SkipNotepad,
    [switch]$SkipBrowser,
    [switch]$SkipTranslation,
    [switch]$RunForeground,
    [string]$ExpectedProcess = "",
    [switch]$ExportBundle,
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

function Test-ExcludedBundlePath {
    param([Parameter(Mandatory = $true)][string]$RelativePath)

    $normalized = $RelativePath -replace "\\", "/"
    if ($normalized -match "(^|/)(recordings|backup|backups)(/|$)") {
        return $true
    }
    if ($normalized -match "(^|/)(model-cache)(/|$)") {
        return $true
    }
    if ($normalized -match "\.(onnx|bin|gguf|safetensors|pt|pth|wav|mp3|flac|m4a|zip)$") {
        return $true
    }
    return $false
}

function Add-BundleFile {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][System.Collections.Generic.List[object]]$Entries,
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$EntryName
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        return
    }
    if (Test-ExcludedBundlePath -RelativePath $EntryName) {
        return
    }
    $Entries.Add([pscustomobject]@{
        source = (Resolve-Path -LiteralPath $Source).Path
        entry  = ($EntryName -replace "\\", "/")
    }) | Out-Null
}

function Get-BundleRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$BaseDir,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $base = (Resolve-Path -LiteralPath $BaseDir).Path -replace '[\\/]+$', ''
    $full = (Resolve-Path -LiteralPath $Path).Path
    if ($full.StartsWith($base, [System.StringComparison]::OrdinalIgnoreCase)) {
        return ($full.Substring($base.Length) -replace '^[\\/]+', '')
    }
    return Split-Path -Leaf $full
}

function New-SupportBundle {
    param(
        [Parameter(Mandatory = $true)][string]$ReportPath,
        [string]$BundlePath = ""
    )

    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    if ([string]::IsNullOrWhiteSpace($BundlePath)) {
        $BundlePath = Join-Path $LogsDir ("target-machine-support-" + $stamp + ".zip")
    }
    $staging = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-target-support-" + [guid]::NewGuid().ToString("N"))
    $entries = [System.Collections.Generic.List[object]]::new()

    try {
        New-Item -ItemType Directory -Path $staging -Force | Out-Null

        Add-BundleFile -Entries $entries -Source $ReportPath -EntryName ("reports/" + (Split-Path -Leaf $ReportPath))
        Add-BundleFile -Entries $entries -Source (Join-Path $AppDir "BUILD.txt") -EntryName "app/BUILD.txt"
        Add-BundleFile -Entries $entries -Source (Join-Path $AppDir "MODEL_ROOT.txt") -EntryName "app/MODEL_ROOT.txt"
        Add-BundleFile -Entries $entries -Source (Join-Path $AppDir "models\MODELS.json") -EntryName "app/models/MODELS.json"
        Add-BundleFile -Entries $entries -Source (Join-Path $AppDir "models\MODELS.md") -EntryName "app/models/MODELS.md"

        $runtimeDir = Join-Path $AppDir ".voice_ime"
        if (Test-Path -LiteralPath $runtimeDir -PathType Container) {
            Get-ChildItem -LiteralPath $runtimeDir -File -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
                $relative = Get-BundleRelativePath -BaseDir $runtimeDir -Path $_.FullName
                Add-BundleFile -Entries $entries -Source $_.FullName -EntryName (Join-Path ".voice_ime" $relative)
            }
        }

        foreach ($item in $entries) {
            $target = Join-Path $staging $item.entry
            New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
            Copy-Item -LiteralPath $item.source -Destination $target -Force
        }

        $summary = [System.Collections.Generic.List[string]]::new()
        $summary.Add("Voice IME Target Machine Support Bundle")
        $summary.Add("created_at=$((Get-Date).ToString("o"))")
        $summary.Add("app_dir=$AppDir")
        $summary.Add("logs_dir=$LogsDir")
        $summary.Add("report=$ReportPath")
        $summary.Add("excluded=recordings, backups, model-cache, archive files, audio/model binary extensions")
        $summary.Add("file_count=$($entries.Count)")
        Set-Content -LiteralPath (Join-Path $staging "summary.txt") -Value ($summary -join [Environment]::NewLine) -Encoding UTF8

        if (Test-Path -LiteralPath $BundlePath -PathType Leaf) {
            Remove-Item -LiteralPath $BundlePath -Force
        }
        Compress-Archive -Path (Join-Path $staging "*") -DestinationPath $BundlePath -CompressionLevel Optimal -Force
        return $BundlePath
    }
    finally {
        if (Test-Path -LiteralPath $staging) {
            Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
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

if ($SkipModelRoot) {
    Add-Row -Name "Model Root" -Status "SKIP" -Detail "skipped by flag"
}
else {
    Invoke-Step -Name "Model Root" -Body {
        Invoke-ToolScript -ScriptName "Model-Root.ps1" | Out-Null
        $report = Get-ChildItem -LiteralPath $LogsDir -Filter "model-root-*.txt" -File -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($report) { return $report.FullName }
        return "model root checked"
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
$shouldExportBundle = ($ExportBundle -or (-not $overall))
$supportBundlePath = ""
if ($shouldExportBundle) {
    $supportBundlePath = Join-Path $LogsDir ("target-machine-support-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".zip")
}

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("Voice IME Target Machine Acceptance")
$lines.Add("created_at=$((Get-Date).ToString("o"))")
$lines.Add("passed=$overall")
$lines.Add("app_dir=$AppDir")
$lines.Add("logs_dir=$LogsDir")
$lines.Add("support_bundle=$supportBundlePath")
$lines.Add("samples_dir=$SamplesDir")
$lines.Add("pass_count=$($passed.Count)")
$lines.Add("fail_count=$($failed.Count)")
$lines.Add("skip_count=$($skipped.Count)")
$lines.Add("")
foreach ($row in $script:Rows) {
    $lines.Add("$($row.status)`t$($row.name)`t$($row.detail)")
}

Set-Content -LiteralPath $ReportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
if ($shouldExportBundle) {
    $supportBundlePath = New-SupportBundle -ReportPath $ReportPath -BundlePath $supportBundlePath
}
Write-Host ""
Write-Host ($lines -join [Environment]::NewLine)
Write-Host "Report: $ReportPath"
if (-not [string]::IsNullOrWhiteSpace($supportBundlePath)) {
    Write-Host "Support bundle: $supportBundlePath"
}

if (-not $NoOpen) {
    Start-Process -FilePath "explorer.exe" -ArgumentList $LogsDir | Out-Null
}

if (-not $overall) {
    exit 2
}
