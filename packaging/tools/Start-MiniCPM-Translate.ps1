$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$HostName = "127.0.0.1"
$Port = 18080
$ModelAlias = "minicpm5-1b-q4"
$ContextSize = "4096"
$Threads = [Math]::Max(2, [Math]::Min(4, [Environment]::ProcessorCount - 1))
$ConfigDir = if ($env:VOICE_IME_APP_DIR) { $env:VOICE_IME_APP_DIR } else { Join-Path $Root ".voice_ime" }
$ConfigPath = Join-Path $ConfigDir "config.json"
$LogPath = Join-Path $ConfigDir "minicpm-translate.log"

function Resolve-RootRelativePath {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) {
        return $null
    }
    $expanded = [Environment]::ExpandEnvironmentVariables($Path.Trim())
    if ([System.IO.Path]::IsPathRooted($expanded)) {
        return $expanded
    }
    return (Join-Path $Root $expanded)
}

function Read-VoiceImeConfig {
    if (-not (Test-Path -LiteralPath $ConfigPath -PathType Leaf)) {
        return [pscustomobject]@{}
    }
    try {
        return Get-Content -LiteralPath $ConfigPath -Raw -Encoding UTF8 | ConvertFrom-Json
    } catch {
        return [pscustomobject]@{}
    }
}

function Resolve-ModelRoot {
    if ($env:VOICE_IME_MODEL_DIR) {
        return Resolve-RootRelativePath -Path $env:VOICE_IME_MODEL_DIR
    }
    $modelRootFile = Join-Path $Root "MODEL_ROOT.txt"
    if (Test-Path -LiteralPath $modelRootFile -PathType Leaf) {
        $fileRoot = Get-Content -LiteralPath $modelRootFile -Encoding UTF8 |
            ForEach-Object { ([string]$_).Trim().TrimStart([char]0xfeff) } |
            Where-Object { $_ -and -not $_.StartsWith("#") } |
            Select-Object -First 1
        if ($fileRoot) {
            return Resolve-RootRelativePath -Path $fileRoot
        }
    }
    $config = Read-VoiceImeConfig
    $configured = $null
    if ($config.asr -and $config.asr.model_root) {
        $configured = [string]$config.asr.model_root
    }
    if ([string]::IsNullOrWhiteSpace($configured) -or $configured -in @("models", "app/models", "app\models", "default", "auto")) {
        return (Join-Path $Root "models")
    }
    return Resolve-RootRelativePath -Path $configured
}

function Get-FirstExistingFile {
    param([string[]]$Paths)
    foreach ($path in $Paths) {
        if ($path -and (Test-Path -LiteralPath $path -PathType Leaf)) {
            return (Get-Item -LiteralPath $path).FullName
        }
    }
    return $null
}

function Get-ShortPath {
    param([string]$Path)
    try {
        if (-not ("ShortPath.Native" -as [type])) {
            Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
namespace ShortPath {
    public static class Native {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern uint GetShortPathName(string longPath, StringBuilder shortPath, uint bufferLength);
    }
}
"@
        }
        $buffer = New-Object System.Text.StringBuilder 512
        $length = [ShortPath.Native]::GetShortPathName($Path, $buffer, 512)
        if ($length -gt 0) {
            $shortPath = $buffer.ToString()
            if ($shortPath -and (Test-Path -LiteralPath $shortPath -PathType Leaf)) {
                return $shortPath
            }
        }
    } catch {
    }
    return $Path
}

function Test-AsciiPath {
    param([string]$Path)
    return ($Path -cmatch '^[\x00-\x7F]+$')
}

function Resolve-ModelPath {
    $modelRoot = Resolve-ModelRoot
    $projectModels = @(
        (Join-Path $modelRoot "minicpm5-1b-q4.gguf"),
        (Join-Path $modelRoot "MiniCPM5-1B-Q4.gguf"),
        (Join-Path $modelRoot "MiniCPM5-1B-Q4_K_M.gguf")
    )
    $model = Get-FirstExistingFile -Paths $projectModels
    if (-not $model -and (Test-Path -LiteralPath "H:\")) {
        $model = Get-ChildItem -LiteralPath "H:\" -File -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like "MiniCPM5-1B-Q4*" } |
            Sort-Object Length |
            Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $model) {
        $deskPetModelDir = Join-Path $env:APPDATA "minicpm-desk-pet\models"
        if (Test-Path -LiteralPath $deskPetModelDir) {
            $model = Get-ChildItem -LiteralPath $deskPetModelDir -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -like "MiniCPM5-1B-*.gguf" } |
                Sort-Object Length |
                Select-Object -First 1 -ExpandProperty FullName
        }
    }
    if (-not $model) {
        throw "MiniCPM GGUF model not found. Put minicpm5-1b-q4.gguf under $modelRoot or set VOICE_IME_MODEL_DIR."
    }

    $runtimeModel = Get-ShortPath -Path $model
    if (Test-AsciiPath -Path $runtimeModel) {
        return @($model, $runtimeModel, $modelRoot)
    }

    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
    $linkPath = Join-Path $ConfigDir "minicpm5-1b-q4.gguf"
    if (-not (Test-Path -LiteralPath $linkPath -PathType Leaf)) {
        try {
            New-Item -ItemType SymbolicLink -Path $linkPath -Target $model | Out-Null
        } catch {
            Write-Host "Short path and symlink failed; copying model once to ASCII path..."
            Copy-Item -LiteralPath $model -Destination $linkPath -Force
        }
    }
    return @($model, $linkPath, $modelRoot)
}

function Resolve-LlamaServer {
    $candidates = @(
        (Join-Path $Root "llama.cpp\cpu\llama-server.exe"),
        (Join-Path $Root "runtime\llama.cpp\cpu\llama-server.exe"),
        "D:\llama.cpp\cpu\llama-server.exe",
        (Join-Path $env:LOCALAPPDATA "Programs\MiniCPM Desk Pet\resources\sidecar-bin\llama-server.exe")
    )
    $server = Get-FirstExistingFile -Paths $candidates
    if (-not $server) {
        throw "llama-server.exe not found. Put llama.cpp cpu build under .\llama.cpp\cpu or D:\llama.cpp\cpu."
    }
    return $server
}

function Test-PortOpen {
    param([string]$HostName, [int]$Port)
    $client = [Net.Sockets.TcpClient]::new()
    try {
        $async = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $async.AsyncWaitHandle.WaitOne(300, $false)) {
            return $false
        }
        $client.EndConnect($async)
        return $true
    } catch {
        return $false
    } finally {
        $client.Close()
    }
}

function Update-VoiceImeConfig {
    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
    $config = Read-VoiceImeConfig
    if (-not $config.translation) {
        $config | Add-Member -Force NoteProperty translation ([pscustomobject]@{})
    }
    $config.translation | Add-Member -Force NoteProperty endpoint "http://$HostName`:$Port/v1/chat/completions"
    $config.translation | Add-Member -Force NoteProperty model $ModelAlias
    $config.translation | Add-Member -Force NoteProperty timeout_seconds 8

    $config | Add-Member -Force NoteProperty translation_endpoint "http://$HostName`:$Port/v1/chat/completions"
    $config | Add-Member -Force NoteProperty translation_model $ModelAlias
    $config | Add-Member -Force NoteProperty translation_timeout 8
    $config | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $ConfigPath -Encoding UTF8
}

New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
$modelInfo = Resolve-ModelPath
$Model = $modelInfo[0]
$RuntimeModel = $modelInfo[1]
$ModelRoot = $modelInfo[2]
$LlamaServer = Resolve-LlamaServer
Update-VoiceImeConfig

Write-Host "===== MiniCPM Translation Server ====="
Write-Host "Model root: $ModelRoot"
Write-Host "Model: $Model"
Write-Host "Runtime model path: $RuntimeModel"
Write-Host "Server: $LlamaServer"
Write-Host "Endpoint: http://$HostName`:$Port/v1/chat/completions"
Write-Host "Log: $LogPath"
Write-Host ""

if (Test-PortOpen -HostName $HostName -Port $Port) {
    Write-Host "Server already running."
    Write-Host "Config OK."
    exit 0
}

Write-Host "Starting llama-server..."
if (Test-Path -LiteralPath $LogPath) {
    Remove-Item -LiteralPath $LogPath -Force
}

$args = @(
    "-m", $RuntimeModel,
    "--host", $HostName,
    "--port", [string]$Port,
    "--alias", $ModelAlias,
    "-c", $ContextSize,
    "-t", [string]$Threads,
    "--gpu-layers", "0",
    "--jinja",
    "--no-webui",
    "--reasoning", "off",
    "--log-file", $LogPath
)
Start-Process -FilePath $LlamaServer -ArgumentList $args -WindowStyle Hidden | Out-Null

Write-Host "Waiting for server..."
$ok = $false
for ($i = 0; $i -lt 90; $i++) {
    try {
        $response = Invoke-WebRequest -UseBasicParsing -Uri "http://$HostName`:$Port/v1/models" -TimeoutSec 1
        if ($response.StatusCode -ge 200) {
            $ok = $true
            break
        }
    } catch {
        Start-Sleep -Seconds 1
    }
}

if ($ok) {
    Write-Host "Server OK."
} else {
    Write-Host "WARNING: Server did not respond."
    if (Test-Path -LiteralPath $LogPath) {
        Write-Host ""
        Write-Host "Last log lines:"
        Get-Content -LiteralPath $LogPath -Tail 80
    }
}
Write-Host "Config OK."
