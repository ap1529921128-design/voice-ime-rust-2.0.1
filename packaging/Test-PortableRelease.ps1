param(
    [string]$ReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable",
    [string]$CoreReleaseRoot = "D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core",
    [string]$ModelPackZip = "D:\voice-ime-build-release\voice-ime-model-pack-asr-fallback-whisper-tiny-int8.zip",
    [switch]$SkipNotepad,
    [switch]$SkipBrowser,
    [switch]$SkipTranslation,
    [switch]$SkipModelPackImport,
    [switch]$KeepRuntimeData
)

$ErrorActionPreference = "Stop"

$LauncherName = ([string][char]21551 + [string][char]21160 + [string][char]35821 + [string][char]38899 + [string][char]36755 + [string][char]20837 + ".bat")

function Assert-PortableLayout {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [switch]$CorePackage
    )

    $resolvedRoot = Resolve-Path -LiteralPath $Root
    $app = Join-Path $resolvedRoot "app"
    $launcher = Join-Path $resolvedRoot $LauncherName
    if (-not (Test-Path -LiteralPath $launcher -PathType Leaf)) {
        throw "Launcher missing: $launcher"
    }
    if (-not (Test-Path -LiteralPath $app -PathType Container)) {
        throw "Hidden app directory missing: $app"
    }
    $appItem = Get-Item -LiteralPath $app -Force
    if (-not (($appItem.Attributes -band [System.IO.FileAttributes]::Hidden) -eq [System.IO.FileAttributes]::Hidden)) {
        throw "App directory is not hidden: $app"
    }
    $visibleRootItems = @(Get-ChildItem -LiteralPath $resolvedRoot)
    if ($visibleRootItems.Count -ne 1 -or $visibleRootItems[0].Name -ne $LauncherName) {
        throw "Root must visibly expose only $LauncherName"
    }
    foreach ($required in @("VoiceIME.exe", "BUILD.txt", "README.md", "models\MODELS.json", "models\MODELS.md")) {
        $path = Join-Path $app $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Required app file missing: $required"
        }
    }

    if ($CorePackage) {
        $modelsDir = Join-Path $app "models"
        $unexpected = @(Get-ChildItem -LiteralPath $modelsDir -Force | Where-Object { $_.Name -notin @("MODELS.json", "MODELS.md") })
        if ($unexpected.Count -gt 0) {
            throw "Core package models directory contains binaries or extra files: $($unexpected.Name -join ', ')"
        }
    }
}

function Assert-BuildStamp {
    param([Parameter(Mandatory = $true)][string]$Root)

    $build = Join-Path $Root "app\BUILD.txt"
    $body = Get-Content -LiteralPath $build -Raw
    foreach ($needle in @("version=2.0.1", "git_status=clean", "git_commit=")) {
        if (-not $body.Contains($needle)) {
            throw "BUILD.txt missing '$needle'"
        }
    }
}

function Invoke-StartupSmoke {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-$Name-startup-" + [guid]::NewGuid().ToString("N"))
        $process = Start-Process -FilePath $exe -WorkingDirectory $app -WindowStyle Hidden -PassThru
        Start-Sleep -Seconds 5
        if ($process.HasExited) {
            throw "$Name startup exited early with code $($process.ExitCode)"
        }
        Stop-Process -Id $process.Id -Force
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    Write-Host "$Name startup smoke passed"
}

function Invoke-DoctorSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-doctor-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList "--doctor" -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Doctor exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "doctor-*.txt" -File -ErrorAction SilentlyContinue)
    if ($reports.Count -eq 0) {
        throw "Doctor did not write a report under $appDir"
    }
    $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $body = Get-Content -LiteralPath $report.FullName -Raw
    if (-not $body.Contains("本地 LLM 文件")) {
        throw "Doctor report does not include local LLM file check"
    }
    Write-Host "Doctor smoke passed: $($report.FullName)"
}

function Invoke-ModelRootFileSmoke {
    param([Parameter(Mandatory = $true)][string]$CoreRoot)

    $tempBase = [System.IO.Path]::GetTempPath()
    $workRoot = Join-Path $tempBase ("voice-ime-model-root-file-" + [guid]::NewGuid().ToString("N"))
    $modelRoot = Join-Path $tempBase ("voice-ime-shared-models-" + [guid]::NewGuid().ToString("N"))
    $appDir = Join-Path $tempBase ("voice-ime-model-root-app-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    $previousModelDir = [Environment]::GetEnvironmentVariable("VOICE_IME_MODEL_DIR", "Process")
    try {
        Copy-Item -LiteralPath $CoreRoot -Destination $workRoot -Recurse -Force
        New-Item -ItemType Directory -Path $modelRoot -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $workRoot "app\MODEL_ROOT.txt") -Value "# shared model repository`n$modelRoot" -Encoding UTF8
        $env:VOICE_IME_APP_DIR = $appDir
        Remove-Item Env:\VOICE_IME_MODEL_DIR -ErrorAction SilentlyContinue
        $exe = Join-Path $workRoot "app\VoiceIME.exe"
        $process = Start-Process -FilePath $exe -ArgumentList "--doctor" -WorkingDirectory (Join-Path $workRoot "app") -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "MODEL_ROOT.txt doctor smoke exited with code $($process.ExitCode)"
        }
        $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "doctor-*.txt" -File -ErrorAction SilentlyContinue)
        if ($reports.Count -eq 0) {
            throw "MODEL_ROOT.txt doctor smoke did not write a report under $appDir"
        }
        $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        $body = Get-Content -LiteralPath $report.FullName -Raw
        if (-not $body.Contains($modelRoot)) {
            throw "Doctor report did not use MODEL_ROOT.txt model root: $modelRoot"
        }
        if (-not $body.Contains("MODEL_ROOT.txt")) {
            throw "Doctor report did not identify MODEL_ROOT.txt as the model root source"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
        if ([string]::IsNullOrEmpty($previousModelDir)) {
            Remove-Item Env:\VOICE_IME_MODEL_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_MODEL_DIR = $previousModelDir
        }
        foreach ($path in @($workRoot, $modelRoot, $appDir)) {
            if ((Test-Path -LiteralPath $path) -and $path.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase)) {
                Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }
    Write-Host "MODEL_ROOT.txt smoke passed"
}

function Invoke-ShutdownSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-shutdown-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList "--shutdown-smoke" -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Shutdown smoke exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    $logs = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "shutdown-*.log" -File -ErrorAction SilentlyContinue)
    if ($logs.Count -eq 0) {
        throw "Shutdown smoke did not write a shutdown log under $appDir"
    }
    $entry = Get-Content -LiteralPath (($logs | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName) |
        Where-Object { $_ -and $_.Trim().Length -gt 0 } |
        Select-Object -Last 1 |
        ConvertFrom-Json
    if (($entry.reason -ne "cli-shutdown-smoke") -or (-not [bool]$entry.history_flushed)) {
        throw "Shutdown smoke log did not report a clean flush"
    }
    Write-Host "Shutdown smoke passed: $appDir"
}

function Invoke-PanicSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-panic-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList "--panic-smoke" -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -eq 0) {
            throw "Panic smoke unexpectedly exited with code 0"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    $logs = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "panic-*.log" -File -ErrorAction SilentlyContinue)
    if ($logs.Count -eq 0) {
        throw "Panic smoke did not write a panic log under $appDir"
    }
    $body = Get-Content -LiteralPath (($logs | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName) -Raw
    if (-not $body.Contains("cli-panic-smoke")) {
        throw "Panic smoke log did not include the panic payload"
    }
    Write-Host "Panic smoke passed: $appDir"
}

function New-TestWavFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    $sampleRate = 16000
    $sampleCount = 1600
    $dataSize = $sampleCount * 2
    $writer = [System.IO.BinaryWriter]::new([System.IO.File]::Create($Path))
    try {
        $ascii = [System.Text.Encoding]::ASCII
        $writer.Write($ascii.GetBytes("RIFF"))
        $writer.Write([int](36 + $dataSize))
        $writer.Write($ascii.GetBytes("WAVE"))
        $writer.Write($ascii.GetBytes("fmt "))
        $writer.Write([int]16)
        $writer.Write([int16]1)
        $writer.Write([int16]1)
        $writer.Write([int]$sampleRate)
        $writer.Write([int]($sampleRate * 2))
        $writer.Write([int16]2)
        $writer.Write([int16]16)
        $writer.Write($ascii.GetBytes("data"))
        $writer.Write([int]$dataSize)
        for ($i = 0; $i -lt $sampleCount; $i++) {
            $writer.Write([int16]0)
        }
    }
    finally {
        $writer.Dispose()
    }
}

function Invoke-AsrBenchmarkProfileSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-asr-benchmark-" + [guid]::NewGuid().ToString("N"))
    $samplesDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-empty-asr-samples-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        New-Item -ItemType Directory -Path $samplesDir -Force | Out-Null
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList @("--benchmark-asr-profile", "fallback", $samplesDir) -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "ASR benchmark profile smoke exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
        if (Test-Path -LiteralPath $samplesDir) {
            Remove-Item -LiteralPath $samplesDir -Recurse -Force
        }
    }
    $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "asr-benchmark-*.csv" -File -ErrorAction SilentlyContinue)
    if ($reports.Count -eq 0) {
        throw "ASR benchmark profile smoke did not write a CSV under $appDir"
    }
    $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $body = Get-Content -LiteralPath $report.FullName -Raw
    if (-not $body.Contains("fallback")) {
        throw "ASR benchmark profile CSV did not record fallback profile"
    }
    if (-not $body.Contains("no wav samples found")) {
        throw "ASR benchmark profile CSV did not record the empty-sample error"
    }
    Write-Host "ASR benchmark profile smoke passed: $($report.FullName)"
}

function Invoke-AsrBenchmarkTemplateSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $samplesDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-asr-template-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-asr-template-app-" + [guid]::NewGuid().ToString("N"))
        $process = Start-Process -FilePath $exe -ArgumentList @("--write-asr-benchmark-template", $samplesDir) -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "ASR benchmark template smoke exited with code $($process.ExitCode)"
        }
        $txtFiles = @(Get-ChildItem -LiteralPath $samplesDir -Filter "*.txt" -File)
        if ($txtFiles.Count -ne 10) {
            throw "ASR benchmark template expected 10 txt files, got $($txtFiles.Count)"
        }
        foreach ($name in @("001.txt", "010.txt", "README.md")) {
            if (-not (Test-Path -LiteralPath (Join-Path $samplesDir $name) -PathType Leaf)) {
                throw "ASR benchmark template missing $name"
            }
        }
        $readme = Get-Content -LiteralPath (Join-Path $samplesDir "README.md") -Raw
        foreach ($needle in @("--benchmark-asr-profile fast", "--benchmark-asr-profile balanced", "--benchmark-asr-profile accurate")) {
            if (-not $readme.Contains($needle)) {
                throw "ASR benchmark template README missing '$needle'"
            }
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
        if (Test-Path -LiteralPath $samplesDir) {
            Remove-Item -LiteralPath $samplesDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Host "ASR benchmark template smoke passed"
}

function Invoke-AsrBenchmarkToolSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $script = Join-Path $Root "app\tools\ASR-Benchmark.ps1"
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "ASR benchmark helper missing: $script"
    }
    $samplesDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-asr-tool-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        $env:VOICE_IME_APP_DIR = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-asr-tool-app-" + [guid]::NewGuid().ToString("N"))
        & powershell -NoProfile -ExecutionPolicy Bypass -File $script -SamplesDir $samplesDir -TemplateOnly -NoOpen
        if ($LASTEXITCODE -ne 0) {
            throw "ASR benchmark helper exited with code $LASTEXITCODE"
        }
        $txtFiles = @(Get-ChildItem -LiteralPath $samplesDir -Filter "*.txt" -File)
        if ($txtFiles.Count -ne 10) {
            throw "ASR benchmark helper expected 10 txt files, got $($txtFiles.Count)"
        }
        if (-not (Test-Path -LiteralPath (Join-Path $samplesDir "README.md") -PathType Leaf)) {
            throw "ASR benchmark helper did not write README.md"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
        if (Test-Path -LiteralPath $samplesDir) {
            Remove-Item -LiteralPath $samplesDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Host "ASR benchmark helper smoke passed"
}

function Invoke-MockAsrBenchmarkSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-mock-asr-" + [guid]::NewGuid().ToString("N"))
    $samplesDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-mock-asr-samples-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        New-Item -ItemType Directory -Path $appDir -Force | Out-Null
        New-Item -ItemType Directory -Path $samplesDir -Force | Out-Null
        $config = @{
            asr = @{
                default_engine = "mock"
                profile = "balanced"
                worker_mode = "isolated"
            }
            smart = @{
                enabled = $false
            }
        } | ConvertTo-Json -Depth 6
        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
        $mockText = ([string][char]38750 + [string][char]27954 + [string][char]20043 + [string][char]26143 + [string][char]21644 + [string][char]28023 + [string][char]27915 + [string][char]20043 + [string][char]27882)
        [System.IO.File]::WriteAllText((Join-Path $appDir "config.json"), $config, $utf8NoBom)
        New-TestWavFile -Path (Join-Path $samplesDir "001.wav")
        [System.IO.File]::WriteAllText((Join-Path $samplesDir "001.txt"), $mockText, $utf8NoBom)
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList @("--benchmark-asr-profile", "balanced", $samplesDir) -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Mock ASR benchmark smoke exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
        if (Test-Path -LiteralPath $samplesDir) {
            Remove-Item -LiteralPath $samplesDir -Recurse -Force
        }
    }
    $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "asr-benchmark-*.csv" -File -ErrorAction SilentlyContinue)
    if ($reports.Count -eq 0) {
        throw "Mock ASR benchmark smoke did not write a CSV under $appDir"
    }
    $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $body = [System.IO.File]::ReadAllText($report.FullName, [System.Text.Encoding]::UTF8)
    $mockText = ([string][char]38750 + [string][char]27954 + [string][char]20043 + [string][char]26143 + [string][char]21644 + [string][char]28023 + [string][char]27915 + [string][char]20043 + [string][char]27882)
    foreach ($needle in @("mock-asr", "mock/balanced", "1.0000", $mockText)) {
        if (-not $body.Contains($needle)) {
            throw "Mock ASR benchmark CSV missing '$needle'"
        }
    }
    Write-Host "Mock ASR benchmark smoke passed: $($report.FullName)"
}

function Invoke-AccurateExternalAsrSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $mockAsr = Join-Path $app "tools\Mock-External-Asr.ps1"
    if (-not (Test-Path -LiteralPath $mockAsr -PathType Leaf)) {
        throw "Mock external ASR helper missing: $mockAsr"
    }
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-accurate-asr-" + [guid]::NewGuid().ToString("N"))
    $samplesDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-accurate-asr-samples-" + [guid]::NewGuid().ToString("N"))
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        New-Item -ItemType Directory -Path $appDir -Force | Out-Null
        New-Item -ItemType Directory -Path $samplesDir -Force | Out-Null
        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
        $mockText = ([string][char]38750 + [string][char]27954 + [string][char]20043 + [string][char]26143 + [string][char]21644 + [string][char]28023 + [string][char]27915 + [string][char]20043 + [string][char]27882)
        $command = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$mockAsr`""
        $config = @{
            asr = @{
                profile = "accurate"
                worker_mode = "isolated"
                accurate_external_command = $command
            }
            smart = @{
                enabled = $false
            }
        } | ConvertTo-Json -Depth 6
        [System.IO.File]::WriteAllText((Join-Path $appDir "config.json"), $config, $utf8NoBom)
        New-TestWavFile -Path (Join-Path $samplesDir "001.wav")
        [System.IO.File]::WriteAllText((Join-Path $samplesDir "001.txt"), $mockText, $utf8NoBom)
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList @("--benchmark-asr-profile", "accurate", $samplesDir) -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Accurate external ASR smoke exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
        if (Test-Path -LiteralPath $samplesDir) {
            Remove-Item -LiteralPath $samplesDir -Recurse -Force
        }
    }
    $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "asr-benchmark-*.csv" -File -ErrorAction SilentlyContinue)
    if ($reports.Count -eq 0) {
        throw "Accurate external ASR smoke did not write a CSV under $appDir"
    }
    $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $body = [System.IO.File]::ReadAllText($report.FullName, [System.Text.Encoding]::UTF8)
    $mockText = ([string][char]38750 + [string][char]27954 + [string][char]20043 + [string][char]26143 + [string][char]21644 + [string][char]28023 + [string][char]27915 + [string][char]20043 + [string][char]27882)
    foreach ($needle in @("accurate", "external-asr", "accurate/external", "1.0000", $mockText)) {
        if (-not $body.Contains($needle)) {
            throw "Accurate external ASR CSV missing '$needle'"
        }
    }
    Write-Host "Accurate external ASR smoke passed: $($report.FullName)"
}

function Invoke-TranslationProfileCliSmoke {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Join-Path $Root "app"
    $exe = Join-Path $app "VoiceIME.exe"
    $mockTranslator = Join-Path $app "tools\Mock-External-Translate.ps1"
    if (-not (Test-Path -LiteralPath $mockTranslator -PathType Leaf)) {
        throw "Mock external translator helper missing: $mockTranslator"
    }
    $appDir = Join-Path ([System.IO.Path]::GetTempPath()) ("voice-ime-translation-profile-" + [guid]::NewGuid().ToString("N"))
    $samplePath = Join-Path $appDir "translation-samples.tsv"
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        New-Item -ItemType Directory -Path $appDir -Force | Out-Null
        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
        $command = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$mockTranslator`""
        $config = @{
            translation = @{
                engine = "external"
                profile = "fast"
                timeout_seconds = 3
                models = @{
                    fast_command = $command
                    balanced_command = ""
                    accurate_command = ""
                }
            }
        } | ConvertTo-Json -Depth 8
        [System.IO.File]::WriteAllText((Join-Path $appDir "config.json"), $config, $utf8NoBom)
        [System.IO.File]::WriteAllText($samplePath, "en`tsettings page local service`tLocal", $utf8NoBom)
        $env:VOICE_IME_APP_DIR = $appDir
        $process = Start-Process -FilePath $exe -ArgumentList @("--benchmark-translation-profile", "fast", $samplePath) -WorkingDirectory $app -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Translation profile CLI smoke exited with code $($process.ExitCode)"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
    $reports = @(Get-ChildItem -LiteralPath (Join-Path $appDir "logs") -Filter "translation-benchmark-*.csv" -File -ErrorAction SilentlyContinue)
    if ($reports.Count -eq 0) {
        throw "Translation profile CLI smoke did not write a CSV under $appDir"
    }
    $report = $reports | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $body = [System.IO.File]::ReadAllText($report.FullName, [System.Text.Encoding]::UTF8)
    foreach ($needle in @("external", "mt/fast", "true", "Local")) {
        if (-not $body.Contains($needle)) {
            throw "Translation profile CLI CSV missing '$needle'"
        }
    }
    Write-Host "Translation profile CLI smoke passed: $($report.FullName)"
}

function Invoke-AcceptanceScript {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$ScriptName
    )

    $script = Join-Path $Root "app\tools\$ScriptName"
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "Acceptance script missing: $script"
    }
    $previousAppDir = [Environment]::GetEnvironmentVariable("VOICE_IME_APP_DIR", "Process")
    try {
        Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        & powershell -NoProfile -ExecutionPolicy Bypass -File $script
        if ($LASTEXITCODE -ne 0) {
            throw "$ScriptName failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        if ([string]::IsNullOrEmpty($previousAppDir)) {
            Remove-Item Env:\VOICE_IME_APP_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:VOICE_IME_APP_DIR = $previousAppDir
        }
    }
}

function Invoke-ModelPackImportAcceptance {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$CoreRoot,
        [Parameter(Mandatory = $true)][string]$PackZip
    )

    $script = Join-Path $Root "app\tools\Model-Pack-Import-Acceptance.ps1"
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "Acceptance script missing: $script"
    }
    & powershell -NoProfile -ExecutionPolicy Bypass -File $script `
        -CoreReleaseRoot $CoreRoot `
        -ModelPackZip $PackZip
    if ($LASTEXITCODE -ne 0) {
        throw "Model-Pack-Import-Acceptance.ps1 failed with exit code $LASTEXITCODE"
    }
}

function Remove-PackageRuntimeData {
    param([Parameter(Mandatory = $true)][string]$Root)

    $app = Resolve-Path -LiteralPath (Join-Path $Root "app")
    $target = Join-Path $app ".voice_ime"
    if (-not (Test-Path -LiteralPath $target)) {
        return
    }
    $resolved = Resolve-Path -LiteralPath $target
    if (-not $resolved.Path.StartsWith($app.Path, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove runtime data outside package app: $($resolved.Path)"
    }
    Remove-Item -LiteralPath $resolved.Path -Recurse -Force
    Write-Host "Removed package runtime data: $($resolved.Path)"
}

if (-not $KeepRuntimeData) {
    Remove-PackageRuntimeData -Root $ReleaseRoot
    Remove-PackageRuntimeData -Root $CoreReleaseRoot
}

Assert-PortableLayout -Root $ReleaseRoot
Assert-PortableLayout -Root $CoreReleaseRoot -CorePackage
Assert-BuildStamp -Root $ReleaseRoot
Assert-BuildStamp -Root $CoreReleaseRoot
Invoke-StartupSmoke -Root $ReleaseRoot -Name "full"
Invoke-StartupSmoke -Root $CoreReleaseRoot -Name "core"
Invoke-DoctorSmoke -Root $ReleaseRoot
Invoke-ModelRootFileSmoke -CoreRoot $CoreReleaseRoot
Invoke-ShutdownSmoke -Root $ReleaseRoot
Invoke-PanicSmoke -Root $ReleaseRoot
Invoke-AsrBenchmarkProfileSmoke -Root $ReleaseRoot
Invoke-AsrBenchmarkTemplateSmoke -Root $ReleaseRoot
Invoke-AsrBenchmarkToolSmoke -Root $ReleaseRoot
Invoke-MockAsrBenchmarkSmoke -Root $ReleaseRoot
Invoke-AccurateExternalAsrSmoke -Root $ReleaseRoot
Invoke-TranslationProfileCliSmoke -Root $ReleaseRoot
if (-not $SkipNotepad) {
    Invoke-AcceptanceScript -Root $ReleaseRoot -ScriptName "Notepad-Input-Acceptance.ps1"
}
if (-not $SkipBrowser) {
    Invoke-AcceptanceScript -Root $ReleaseRoot -ScriptName "Browser-Input-Acceptance.ps1"
}
if (-not $SkipTranslation) {
    Invoke-AcceptanceScript -Root $ReleaseRoot -ScriptName "Translation-Acceptance.ps1"
}
if (-not $SkipModelPackImport) {
    Invoke-ModelPackImportAcceptance -Root $ReleaseRoot -CoreRoot $CoreReleaseRoot -PackZip $ModelPackZip
}
if (-not $KeepRuntimeData) {
    Remove-PackageRuntimeData -Root $ReleaseRoot
    Remove-PackageRuntimeData -Root $CoreReleaseRoot
}

Write-Host "Portable release verification passed"
