param(
    [string]$Text = ("Voice IME foreground acceptance " + (Get-Date -Format "yyyyMMdd-HHmmss")),
    [int]$DelayMs = 80,
    [int]$CountdownSeconds = 5,
    [string]$ExpectedProcess = "",
    [string]$ExpectedClassContains = "",
    [string]$ExpectedTitleContains = ""
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$LogsDir = Join-Path $AppDir ".voice_ime\logs"
$ReportPath = Join-Path $LogsDir ("foreground-acceptance-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}
New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null

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

function Get-LatestInputTarget {
    $targetLog = Get-ChildItem -LiteralPath $LogsDir -Filter "input-target-*.log" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $targetLog) {
        return $null
    }
    $line = Get-Content -LiteralPath $targetLog.FullName -ErrorAction SilentlyContinue |
        Where-Object { $_ -and $_.Trim().Length -gt 0 } |
        Select-Object -Last 1
    if (-not $line) {
        return $null
    }
    try {
        return $line | ConvertFrom-Json
    }
    catch {
        return $null
    }
}

function Split-ExpectedProcess {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return @()
    }
    return @($Value -split '[,;]' | ForEach-Object { $_.Trim() } | Where-Object { $_.Length -gt 0 })
}

function Test-ExpectedProcess {
    param(
        [string]$Actual,
        [string[]]$Expected
    )
    if ($Expected.Count -eq 0) {
        return $true
    }
    foreach ($item in $Expected) {
        if ($Actual -ieq $item) {
            return $true
        }
        if ($Actual -ieq "$item.exe") {
            return $true
        }
    }
    return $false
}

$expectedProcesses = Split-ExpectedProcess -Value $ExpectedProcess

Write-Host "Voice IME Foreground Acceptance"
Write-Host "Focus the target input box now. The paste starts after $CountdownSeconds seconds."
Write-Host "Text: $Text"
for ($second = $CountdownSeconds; $second -gt 0; $second -= 1) {
    Write-Host "$second..."
    Start-Sleep -Seconds 1
}

$argumentList = @(
    (Quote-ProcessArgument "--paste-foreground"),
    (Quote-ProcessArgument $Text),
    (Quote-ProcessArgument ([string]$DelayMs))
) -join " "

$paste = Start-Process -FilePath $Exe `
    -ArgumentList $argumentList `
    -WorkingDirectory $AppDir `
    -PassThru `
    -Wait `
    -WindowStyle Hidden

$targetEntry = Get-LatestInputTarget
$targetProcess = if ($targetEntry) { [string]$targetEntry.target.process_name } else { "" }
$targetClass = if ($targetEntry) { [string]$targetEntry.target.class_name } else { "" }
$targetTitle = if ($targetEntry) { [string]$targetEntry.target.title } else { "" }
$caretSource = if ($targetEntry) { [string]$targetEntry.target.caret_source } else { "" }
$rect = if ($targetEntry -and $targetEntry.target.rect) { ($targetEntry.target.rect | ConvertTo-Json -Compress) } else { "" }
$inputMethod = if ($targetEntry) { [string]$targetEntry.input_method } else { "" }
$sendInputEvents = if ($targetEntry) { [string]$targetEntry.send_input_events } else { "" }
$focusAttempts = if ($targetEntry) { [string]$targetEntry.focus_attempts } else { "" }
$focusRestored = if ($targetEntry) { [string]$targetEntry.focus_restored } else { "" }
$clipboardPreviousFormat = if ($targetEntry) { [string]$targetEntry.clipboard_previous_format } else { "" }
$clipboardPreviousHadText = if ($targetEntry) { [string]$targetEntry.clipboard_previous_had_text } else { "" }
$clipboardFormatCount = if ($targetEntry) { [string]$targetEntry.clipboard_format_count } else { "" }
$clipboardSequenceBefore = if ($targetEntry) { [string]$targetEntry.clipboard_sequence_before } else { "" }
$clipboardSequenceAfter = if ($targetEntry) { [string]$targetEntry.clipboard_sequence_after } else { "" }
$clipboardRestored = if ($targetEntry) { [string]$targetEntry.clipboard_restored } else { "" }
$pasteResult = if ($targetEntry) { [string]$targetEntry.result } else { "" }
$pasteError = if ($targetEntry) { [string]$targetEntry.error } else { "" }

$targetProcessOk = Test-ExpectedProcess -Actual $targetProcess -Expected $expectedProcesses
$targetClassOk = [string]::IsNullOrWhiteSpace($ExpectedClassContains) -or $targetClass.Contains($ExpectedClassContains)
$targetTitleOk = [string]::IsNullOrWhiteSpace($ExpectedTitleContains) -or $targetTitle.Contains($ExpectedTitleContains)
$targetOk = $targetProcessOk -and $targetClassOk -and $targetTitleOk
$passed = ($paste.ExitCode -eq 0) -and $targetOk

$lines = @(
    "Voice IME Foreground Acceptance",
    "created_at=$((Get-Date).ToString("o"))",
    "passed=$passed",
    "manual_content_check_required=True",
    "paste_exit_code=$($paste.ExitCode)",
    "target_ok=$targetOk",
    "expected_process=$ExpectedProcess",
    "expected_class_contains=$ExpectedClassContains",
    "expected_title_contains=$ExpectedTitleContains",
    "target_process=$targetProcess",
    "target_class=$targetClass",
    "target_title=$targetTitle",
    "caret_source=$caretSource",
    "rect=$rect",
    "input_method=$inputMethod",
    "send_input_events=$sendInputEvents",
    "focus_attempts=$focusAttempts",
    "focus_restored=$focusRestored",
    "clipboard_previous_format=$clipboardPreviousFormat",
    "clipboard_previous_had_text=$clipboardPreviousHadText",
    "clipboard_format_count=$clipboardFormatCount",
    "clipboard_sequence_before=$clipboardSequenceBefore",
    "clipboard_sequence_after=$clipboardSequenceAfter",
    "clipboard_restored=$clipboardRestored",
    "paste_result=$pasteResult",
    "paste_error=$pasteError",
    "text=$Text",
    "logs_dir=$LogsDir"
)

Set-Content -LiteralPath $ReportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
Write-Host ($lines -join [Environment]::NewLine)
if (-not $passed) {
    exit 2
}
