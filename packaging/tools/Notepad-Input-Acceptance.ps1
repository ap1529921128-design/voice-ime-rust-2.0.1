param(
    [string]$Text = ("Voice IME Notepad acceptance " + (Get-Date -Format "yyyyMMdd-HHmmss"))
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$LogsDir = Join-Path $AppDir ".voice_ime\logs"
$ReportPath = Join-Path $LogsDir ("notepad-acceptance-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}
New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null

if (-not ("VoiceImeWin32" -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class VoiceImeWin32 {
  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")]
  public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")]
  public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll", SetLastError=true)]
  public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
  [DllImport("user32.dll")]
  public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")]
  public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
  [DllImport("user32.dll")]
  public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
}
public struct RECT {
  public int Left;
  public int Top;
  public int Right;
  public int Bottom;
}
"@
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

function Wait-MainWindow {
    param(
        [System.Diagnostics.Process]$Process,
        [string]$TitleFragment,
        [int[]]$BaselineIds
    )
    $deadline = (Get-Date).AddSeconds(12)
    while ((Get-Date) -lt $deadline) {
        if ($Process -and -not $Process.HasExited) {
            $Process.Refresh()
            if ($Process.MainWindowHandle -ne 0) {
                return $Process
            }
        }
        $candidate = Get-Process -Name "Notepad" -ErrorAction SilentlyContinue |
            Where-Object {
                $_.MainWindowHandle -ne 0 -and (
                    ($_.MainWindowTitle -and $_.MainWindowTitle.Contains($TitleFragment)) -or
                    ($BaselineIds -notcontains $_.Id)
                )
            } |
            Select-Object -First 1
        if ($candidate) {
            return $candidate
        }
        Start-Sleep -Milliseconds 150
    }
    throw "Notepad window did not appear"
}

function Focus-Window {
    param([System.Diagnostics.Process]$Process)
    $Process.Refresh()
    if ($Process.MainWindowHandle -eq 0) {
        return
    }
    [VoiceImeWin32]::ShowWindow($Process.MainWindowHandle, 9) | Out-Null
    [VoiceImeWin32]::SetWindowPos($Process.MainWindowHandle, [IntPtr]::new(-1), 0, 0, 0, 0, 0x0043) | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    for ($attempt = 0; $attempt -lt 4; $attempt += 1) {
        $shell.AppActivate($Process.Id) | Out-Null
        [VoiceImeWin32]::SetForegroundWindow($Process.MainWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 250
        $rect = New-Object RECT
        if ([VoiceImeWin32]::GetWindowRect($Process.MainWindowHandle, [ref]$rect)) {
            $x = [int](($rect.Left + $rect.Right) / 2)
            $y = [int](($rect.Top + $rect.Bottom) / 2)
            [VoiceImeWin32]::SetCursorPos($x, $y) | Out-Null
            Start-Sleep -Milliseconds 80
            [VoiceImeWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
            [VoiceImeWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        }
        Start-Sleep -Milliseconds 250
        if ([VoiceImeWin32]::GetForegroundWindow() -eq $Process.MainWindowHandle) {
            return
        }
    }
}

function Clear-WindowTopmost {
    param([System.Diagnostics.Process]$Process)
    if (-not $Process -or $Process.HasExited) {
        return
    }
    $Process.Refresh()
    if ($Process.MainWindowHandle -ne 0) {
        [VoiceImeWin32]::SetWindowPos($Process.MainWindowHandle, [IntPtr]::new(-2), 0, 0, 0, 0, 0x0003) | Out-Null
    }
}

function Send-Key {
    param([byte]$VirtualKey)
    [VoiceImeWin32]::keybd_event($VirtualKey, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [VoiceImeWin32]::keybd_event($VirtualKey, 0, 0x0002, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 80
}

function Send-CtrlKey {
    param([byte]$VirtualKey)
    [VoiceImeWin32]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [VoiceImeWin32]::keybd_event($VirtualKey, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [VoiceImeWin32]::keybd_event($VirtualKey, 0, 0x0002, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [VoiceImeWin32]::keybd_event(0x11, 0, 0x0002, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 120
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

$previousClipboard = $null
try {
    $previousClipboard = Get-Clipboard -Raw -ErrorAction SilentlyContinue
}
catch {
    $previousClipboard = $null
}

$notepad = $null
$tempFile = Join-Path $env:TEMP ("voice-ime-acceptance-" + [guid]::NewGuid().ToString("N") + ".txt")
try {
    Set-Content -LiteralPath $tempFile -Value "" -Encoding UTF8
    $baselineNotepadIds = @(Get-Process -Name "Notepad" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    $notepad = Start-Process -FilePath "notepad.exe" -ArgumentList (Quote-ProcessArgument $tempFile) -PassThru
    $notepad = Wait-MainWindow -Process $notepad -TitleFragment (Split-Path $tempFile -Leaf) -BaselineIds $baselineNotepadIds
    Focus-Window -Process $notepad
    Send-CtrlKey 0x41
    Send-Key 0x2E

    $argumentList = @(
        (Quote-ProcessArgument "--paste-foreground"),
        (Quote-ProcessArgument $Text),
        (Quote-ProcessArgument "80")
    ) -join " "

    $paste = Start-Process -FilePath $Exe `
        -ArgumentList $argumentList `
        -WorkingDirectory $AppDir `
        -PassThru `
        -Wait `
        -WindowStyle Hidden

    $targetEntry = Get-LatestInputTarget
    $targetProcess = if ($targetEntry) { [string]$targetEntry.target.process_name } else { "" }
    $targetTitle = if ($targetEntry) { [string]$targetEntry.target.title } else { "" }
    $caretSource = if ($targetEntry) { [string]$targetEntry.target.caret_source } else { "" }
    $focusAttempts = if ($targetEntry) { [string]$targetEntry.focus_attempts } else { "" }
    $focusRestored = if ($targetEntry) { [string]$targetEntry.focus_restored } else { "" }
    $clipboardPreviousFormat = if ($targetEntry) { [string]$targetEntry.clipboard_previous_format } else { "" }
    $clipboardPreviousHadText = if ($targetEntry) { [string]$targetEntry.clipboard_previous_had_text } else { "" }
    $clipboardRestored = if ($targetEntry) { [string]$targetEntry.clipboard_restored } else { "" }
    $targetOk = ($targetProcess -ieq "notepad.exe")

    Focus-Window -Process $notepad
    Send-CtrlKey 0x41
    Send-CtrlKey 0x43
    Start-Sleep -Milliseconds 500
    $actual = Get-Clipboard -Raw -ErrorAction SilentlyContinue

    $passed = ($paste.ExitCode -eq 0) -and $targetOk -and (($actual -replace "`r`n$", "") -eq $Text)
    $lines = @(
        "Voice IME Notepad Acceptance",
        "created_at=$((Get-Date).ToString("o"))",
        "passed=$passed",
        "paste_exit_code=$($paste.ExitCode)",
        "target_ok=$targetOk",
        "target_process=$targetProcess",
        "target_title=$targetTitle",
        "caret_source=$caretSource",
        "focus_attempts=$focusAttempts",
        "focus_restored=$focusRestored",
        "clipboard_previous_format=$clipboardPreviousFormat",
        "clipboard_previous_had_text=$clipboardPreviousHadText",
        "clipboard_restored=$clipboardRestored",
        "expected=$Text",
        "actual=$actual",
        "logs_dir=$LogsDir"
    )
    Set-Content -LiteralPath $ReportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
    Write-Host ($lines -join [Environment]::NewLine)
    if (-not $passed) {
        exit 2
    }
}
finally {
    Clear-WindowTopmost -Process $notepad
    if ($notepad -and -not $notepad.HasExited) {
        Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $tempFile -Force -ErrorAction SilentlyContinue
    if ($null -ne $previousClipboard) {
        Set-Clipboard -Value $previousClipboard
    }
}
