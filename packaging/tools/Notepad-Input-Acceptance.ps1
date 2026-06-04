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
    $shell = New-Object -ComObject WScript.Shell
    $shell.AppActivate($Process.Id) | Out-Null
    [VoiceImeWin32]::SetForegroundWindow($Process.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 450
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
    $shell = New-Object -ComObject WScript.Shell
    $shell.SendKeys("^a")
    Start-Sleep -Milliseconds 150
    $shell.SendKeys("{DEL}")
    Start-Sleep -Milliseconds 150

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

    Focus-Window -Process $notepad
    $shell.SendKeys("^a")
    Start-Sleep -Milliseconds 150
    $shell.SendKeys("^c")
    Start-Sleep -Milliseconds 500
    $actual = Get-Clipboard -Raw -ErrorAction SilentlyContinue

    $passed = ($paste.ExitCode -eq 0) -and (($actual -replace "`r`n$", "") -eq $Text)
    $lines = @(
        "Voice IME Notepad Acceptance",
        "created_at=$((Get-Date).ToString("o"))",
        "passed=$passed",
        "paste_exit_code=$($paste.ExitCode)",
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
    if ($notepad -and -not $notepad.HasExited) {
        Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $tempFile -Force -ErrorAction SilentlyContinue
    if ($null -ne $previousClipboard) {
        Set-Clipboard -Value $previousClipboard
    }
}
