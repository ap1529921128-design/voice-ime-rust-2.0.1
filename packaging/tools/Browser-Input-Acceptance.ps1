param(
    [string]$Text = ("Voice IME browser acceptance " + (Get-Date -Format "yyyyMMdd-HHmmss")),
    [ValidateSet("auto", "edge", "chrome")]
    [string]$Browser = "auto"
)

$ErrorActionPreference = "Stop"

$AppDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Exe = Join-Path $AppDir "VoiceIME.exe"
$LogsDir = Join-Path $AppDir ".voice_ime\logs"
$ReportPath = Join-Path $LogsDir ("browser-acceptance-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".txt")

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "VoiceIME.exe not found: $Exe"
}
New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null

if (-not ("VoiceImeBrowserWin32" -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class VoiceImeBrowserWin32 {
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

function ConvertTo-FileUri {
    param([string]$Path)
    return ([System.Uri]::new($Path)).AbsoluteUri
}

function Resolve-Browser {
    param([string]$Requested)
    $specs = @(
        @{
            Id = "edge"
            ProcessNames = @("msedge")
            Paths = @(
                (Join-Path ${env:ProgramFiles(x86)} "Microsoft\Edge\Application\msedge.exe"),
                (Join-Path $env:ProgramFiles "Microsoft\Edge\Application\msedge.exe")
            )
        },
        @{
            Id = "chrome"
            ProcessNames = @("chrome")
            Paths = @(
                (Join-Path $env:ProgramFiles "Google\Chrome\Application\chrome.exe"),
                (Join-Path ${env:ProgramFiles(x86)} "Google\Chrome\Application\chrome.exe")
            )
        }
    )

    foreach ($spec in $specs) {
        if ($Requested -ne "auto" -and $spec.Id -ne $Requested) {
            continue
        }
        foreach ($path in $spec.Paths) {
            if ($path -and (Test-Path -LiteralPath $path -PathType Leaf)) {
                return @{
                    Id = $spec.Id
                    Path = $path
                    ProcessNames = $spec.ProcessNames
                }
            }
        }
        foreach ($name in $spec.ProcessNames) {
            $command = Get-Command $name -ErrorAction SilentlyContinue
            if ($command -and $command.Source) {
                return @{
                    Id = $spec.Id
                    Path = $command.Source
                    ProcessNames = $spec.ProcessNames
                }
            }
        }
    }
    throw "No supported browser found. Install Microsoft Edge or Google Chrome, or pass -Browser edge/chrome."
}

function Get-BrowserWindow {
    param(
        [string[]]$ProcessNames,
        [string]$TitleFragment,
        [int[]]$BaselineIds
    )
    foreach ($name in $ProcessNames) {
        $candidate = Get-Process -Name $name -ErrorAction SilentlyContinue |
            Where-Object {
                $_.MainWindowHandle -ne 0 -and
                $_.MainWindowTitle -and
                $_.MainWindowTitle.Contains($TitleFragment) -and
                ($BaselineIds -notcontains $_.Id)
            } |
            Select-Object -First 1
        if ($candidate) {
            return $candidate
        }
    }
    return $null
}

function Wait-BrowserWindow {
    param(
        [string[]]$ProcessNames,
        [string]$TitleFragment,
        [int[]]$BaselineIds
    )
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        $candidate = Get-BrowserWindow -ProcessNames $ProcessNames -TitleFragment $TitleFragment -BaselineIds $BaselineIds
        if ($candidate) {
            return $candidate
        }
        Start-Sleep -Milliseconds 200
    }
    throw "Browser acceptance window did not appear"
}

function Focus-Window {
    param([System.Diagnostics.Process]$Process)
    $Process.Refresh()
    if ($Process.MainWindowHandle -eq 0) {
        return
    }
    [VoiceImeBrowserWin32]::ShowWindow($Process.MainWindowHandle, 9) | Out-Null
    [VoiceImeBrowserWin32]::SetWindowPos($Process.MainWindowHandle, [IntPtr]::new(-1), 0, 0, 0, 0, 0x0043) | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    for ($attempt = 0; $attempt -lt 4; $attempt += 1) {
        $shell.AppActivate($Process.Id) | Out-Null
        [VoiceImeBrowserWin32]::SetForegroundWindow($Process.MainWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 250
        $rect = New-Object RECT
        if ([VoiceImeBrowserWin32]::GetWindowRect($Process.MainWindowHandle, [ref]$rect)) {
            $x = [int](($rect.Left + $rect.Right) / 2)
            $y = [int](($rect.Top + $rect.Bottom) / 2)
            [VoiceImeBrowserWin32]::SetCursorPos($x, $y) | Out-Null
            Start-Sleep -Milliseconds 80
            [VoiceImeBrowserWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
            [VoiceImeBrowserWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        }
        Start-Sleep -Milliseconds 250
        if ([VoiceImeBrowserWin32]::GetForegroundWindow() -eq $Process.MainWindowHandle) {
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
        [VoiceImeBrowserWin32]::SetWindowPos($Process.MainWindowHandle, [IntPtr]::new(-2), 0, 0, 0, 0, 0x0003) | Out-Null
    }
}

function Wait-TitleContains {
    param(
        [string[]]$ProcessNames,
        [string]$TitleFragment,
        [int[]]$BaselineIds,
        [string]$Expected,
        [int]$TimeoutSeconds = 8
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $latest = ""
    while ((Get-Date) -lt $deadline) {
        $candidate = Get-BrowserWindow -ProcessNames $ProcessNames -TitleFragment $TitleFragment -BaselineIds $BaselineIds
        if ($candidate) {
            $latest = $candidate.MainWindowTitle
            if ($latest.Contains($Expected)) {
                return @{
                    Passed = $true
                    Title = $latest
                }
            }
        }
        Start-Sleep -Milliseconds 150
    }
    return @{
        Passed = $false
        Title = $latest
    }
}

function Stop-BrowserProfileProcesses {
    param(
        [string[]]$ProcessNames,
        [string]$ProfileDir
    )
    foreach ($name in $ProcessNames) {
        Get-CimInstance Win32_Process -Filter "name='$name.exe'" -ErrorAction SilentlyContinue |
            Where-Object { $_.CommandLine -and $_.CommandLine.Contains($ProfileDir) } |
            ForEach-Object {
                Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
            }
    }
}

function Wait-ProcessWithFocus {
    param(
        [System.Diagnostics.Process]$Process,
        [string[]]$ProcessNames,
        [string]$TitleFragment,
        [int[]]$BaselineIds,
        [int]$TimeoutSeconds = 12
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (-not $Process.HasExited -and (Get-Date) -lt $deadline) {
        $focusWindow = Get-BrowserWindow -ProcessNames $ProcessNames -TitleFragment $TitleFragment -BaselineIds $BaselineIds
        if ($focusWindow -and $focusWindow.MainWindowHandle -ne 0) {
            [VoiceImeBrowserWin32]::ShowWindow($focusWindow.MainWindowHandle, 9) | Out-Null
            [VoiceImeBrowserWin32]::SetWindowPos($focusWindow.MainWindowHandle, [IntPtr]::new(-1), 0, 0, 0, 0, 0x0043) | Out-Null
            [VoiceImeBrowserWin32]::SetForegroundWindow($focusWindow.MainWindowHandle) | Out-Null
        }
        Start-Sleep -Milliseconds 80
        $Process.Refresh()
    }
    if (-not $Process.HasExited) {
        $Process.WaitForExit(5000) | Out-Null
    }
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
$previousInputTargetHwnd = [Environment]::GetEnvironmentVariable("VOICE_IME_INPUT_TARGET_HWND", "Process")
$previousTargetedUiaValue = [Environment]::GetEnvironmentVariable("VOICE_IME_ALLOW_TARGETED_UIA_VALUE", "Process")
try {
    $previousClipboard = Get-Clipboard -Raw -ErrorAction SilentlyContinue
}
catch {
    $previousClipboard = $null
}

$browserSpec = Resolve-Browser -Requested $Browser
$id = [guid]::NewGuid().ToString("N")
$titleToken = "VoiceIME Browser Acceptance $id"
$tempRoot = Join-Path $env:TEMP ("voice-ime-browser-acceptance-" + $id)
$profileDir = Join-Path $tempRoot "profile"
$htmlPath = Join-Path $tempRoot "index.html"
$browserWindow = $null

try {
    New-Item -ItemType Directory -Path $profileDir -Force | Out-Null
    $titleJson = $titleToken | ConvertTo-Json -Compress
    $htmlTitle = [System.Net.WebUtility]::HtmlEncode($titleToken)
    $html = @"
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>$htmlTitle READY</title>
  <style>
    html, body { margin: 0; width: 100%; height: 100%; background: #f5f7fb; }
    body { display: grid; place-items: center; font: 16px/1.4 system-ui, sans-serif; }
    textarea {
      width: min(680px, calc(100vw - 48px));
      height: min(280px, calc(100vh - 48px));
      border: 1px solid #8aa0c8;
      border-radius: 10px;
      padding: 18px;
      outline: none;
      resize: none;
      color: #111827;
      background: white;
      box-shadow: 0 18px 48px rgba(31, 41, 55, .16);
    }
  </style>
</head>
<body>
  <textarea id="target" autofocus spellcheck="false"></textarea>
  <script>
    const base = $titleJson;
    const box = document.getElementById('target');
    function syncTitle() {
      document.title = base + ' :: ' + box.value;
    }
    function focusTarget() {
      box.focus({ preventScroll: true });
      box.select();
    }
    window.addEventListener('load', () => {
      focusTarget();
      syncTitle();
      let ticks = 0;
      const timer = setInterval(() => {
        focusTarget();
        ticks += 1;
        syncTitle();
        if (ticks > 40 || box.value.length > 0) clearInterval(timer);
      }, 100);
    });
    box.addEventListener('input', syncTitle);
  </script>
</body>
</html>
"@
    Set-Content -LiteralPath $htmlPath -Value $html -Encoding UTF8

    $baselineIds = @()
    foreach ($name in $browserSpec.ProcessNames) {
        $baselineIds += @(Get-Process -Name $name -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    }

    $browserArgs = @(
        "--user-data-dir=$(Quote-ProcessArgument $profileDir)",
        "--no-first-run",
        "--disable-default-apps",
        "--disable-session-crashed-bubble",
        "--force-renderer-accessibility",
        "--new-window",
        (Quote-ProcessArgument (ConvertTo-FileUri $htmlPath))
    ) -join " "
    Start-Process -FilePath $browserSpec.Path -ArgumentList $browserArgs -WorkingDirectory $tempRoot | Out-Null

    $browserWindow = Wait-BrowserWindow -ProcessNames $browserSpec.ProcessNames -TitleFragment $titleToken -BaselineIds $baselineIds
    Focus-Window -Process $browserWindow
    $env:VOICE_IME_INPUT_TARGET_HWND = [string]$browserWindow.MainWindowHandle.ToInt64()
    $env:VOICE_IME_ALLOW_TARGETED_UIA_VALUE = "1"

    $pasteExitCode = -1
    $pasteAttempts = 0
    $titleResult = @{
        Passed = $false
        Title = ""
    }
    foreach ($pasteDelay in @(180, 520)) {
        $pasteAttempts += 1
        Focus-Window -Process $browserWindow
        $argumentList = @(
            (Quote-ProcessArgument "--paste-foreground"),
            (Quote-ProcessArgument $Text),
            (Quote-ProcessArgument ([string]$pasteDelay))
        ) -join " "
        $paste = Start-Process -FilePath $Exe `
            -ArgumentList $argumentList `
            -WorkingDirectory $AppDir `
            -PassThru `
            -WindowStyle Hidden
        Wait-ProcessWithFocus -Process $paste -ProcessNames $browserSpec.ProcessNames -TitleFragment $titleToken -BaselineIds $baselineIds
        $pasteExitCode = $paste.ExitCode
        $titleResult = Wait-TitleContains -ProcessNames $browserSpec.ProcessNames -TitleFragment $titleToken -BaselineIds $baselineIds -Expected $Text -TimeoutSeconds 8
        if (($pasteExitCode -eq 0) -and $titleResult.Passed) {
            break
        }
        Start-Sleep -Milliseconds 350
    }
    $targetEntry = Get-LatestInputTarget
    $targetProcess = if ($targetEntry) { [string]$targetEntry.target.process_name } else { "" }
    $targetTitle = if ($targetEntry) { [string]$targetEntry.target.title } else { "" }
    $caretSource = if ($targetEntry) { [string]$targetEntry.target.caret_source } else { "" }
    $focusAttempts = if ($targetEntry) { [string]$targetEntry.focus_attempts } else { "" }
    $focusRestored = if ($targetEntry) { [string]$targetEntry.focus_restored } else { "" }
    $clipboardPreviousFormat = if ($targetEntry) { [string]$targetEntry.clipboard_previous_format } else { "" }
    $clipboardPreviousHadText = if ($targetEntry) { [string]$targetEntry.clipboard_previous_had_text } else { "" }
    $clipboardRestored = if ($targetEntry) { [string]$targetEntry.clipboard_restored } else { "" }
    $expectedProcesses = @($browserSpec.ProcessNames | ForEach-Object { "$_.exe" })
    $targetOk = @($expectedProcesses | Where-Object { $_ -ieq $targetProcess }).Count -gt 0
    $passed = ($pasteExitCode -eq 0) -and $targetOk -and $titleResult.Passed
    $lines = @(
        "Voice IME Browser Acceptance",
        "created_at=$((Get-Date).ToString("o"))",
        "passed=$passed",
        "browser=$($browserSpec.Id)",
        "paste_exit_code=$pasteExitCode",
        "paste_attempts=$pasteAttempts",
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
        "window_title=$($titleResult.Title)",
        "logs_dir=$LogsDir"
    )
    Set-Content -LiteralPath $ReportPath -Value ($lines -join [Environment]::NewLine) -Encoding UTF8
    Write-Host ($lines -join [Environment]::NewLine)
    if (-not $passed) {
        exit 2
    }
}
finally {
    Clear-WindowTopmost -Process $browserWindow
    Stop-BrowserProfileProcesses -ProcessNames $browserSpec.ProcessNames -ProfileDir $profileDir
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    if ($null -ne $previousClipboard) {
        Set-Clipboard -Value $previousClipboard
    }
    if ($null -eq $previousInputTargetHwnd) {
        Remove-Item Env:\VOICE_IME_INPUT_TARGET_HWND -ErrorAction SilentlyContinue
    }
    else {
        $env:VOICE_IME_INPUT_TARGET_HWND = $previousInputTargetHwnd
    }
    if ($null -eq $previousTargetedUiaValue) {
        Remove-Item Env:\VOICE_IME_ALLOW_TARGETED_UIA_VALUE -ErrorAction SilentlyContinue
    }
    else {
        $env:VOICE_IME_ALLOW_TARGETED_UIA_VALUE = $previousTargetedUiaValue
    }
}
