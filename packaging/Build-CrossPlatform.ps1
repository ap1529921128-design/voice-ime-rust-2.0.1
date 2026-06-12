param(
    [ValidateSet("windows", "linux", "macos", "android", "ios")]
    [string]$Platform = "",
    [switch]$InitMobile,
    [switch]$NoBundle
)

$ErrorActionPreference = "Stop"
trap {
    Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Resolve-Path (Join-Path $ScriptDir "..")
$Npm = if ($IsWindows -or $env:OS -eq "Windows_NT") { "npm.cmd" } else { "npm" }

function Get-DefaultPlatform {
    if ($IsWindows -or $env:OS -eq "Windows_NT") { return "windows" }
    if ($IsMacOS) { return "macos" }
    if ($IsLinux) { return "linux" }
    throw "Unsupported host OS. Pass -Platform explicitly after installing a supported toolchain."
}

function Assert-Command {
    param([string]$Name, [string]$Hint)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing command '$Name'. $Hint"
    }
}

function Assert-Host {
    param([string]$Required, [string]$Reason)
    $actual = Get-DefaultPlatform
    if ($actual -ne $Required) {
        throw "$Reason Current host is '$actual'; required host is '$Required'."
    }
}

function Invoke-Checked {
    param([string]$File, [string[]]$Arguments)
    Write-Host ">> $File $($Arguments -join ' ')"
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE"
    }
}

function Ensure-NodeAndRust {
    Assert-Command $Npm "Install Node.js and run npm install in the project root."
    Assert-Command "cargo" "Install Rust stable for this host."
    Assert-Command "rustup" "Install rustup and the platform Rust target."
}

function Ensure-Android {
    Assert-Command "java" "Install JDK 17 or newer."
    $sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { $env:ANDROID_SDK_ROOT }
    if ([string]::IsNullOrWhiteSpace($sdk)) {
        throw "ANDROID_HOME or ANDROID_SDK_ROOT is not set. Install Android Studio, SDK and NDK first."
    }
    if (-not (Test-Path -LiteralPath $sdk)) {
        throw "Android SDK path does not exist: $sdk"
    }
}

function Assert-TauriSubcommand {
    param([string]$Subcommand)
    $output = & $Npm run tauri -- --help 2>&1
    $text = $output -join [Environment]::NewLine
    if ($LASTEXITCODE -ne 0 -or ($text -notmatch "(?m)^\s+$([regex]::Escape($Subcommand))\s+")) {
        throw "This Tauri CLI does not expose '$Subcommand'. Update @tauri-apps/cli on the target host or follow the current Tauri mobile setup guide."
    }
}

Set-Location $Root
if ([string]::IsNullOrWhiteSpace($Platform)) {
    $Platform = Get-DefaultPlatform
}

Ensure-NodeAndRust

switch ($Platform) {
    "windows" {
        Assert-Host "windows" "Windows installer and portable package must be built on Windows."
        $args = @("run", "tauri", "build")
        if ($NoBundle) { $args += "--"; $args += "--no-bundle" }
        Invoke-Checked $Npm $args
        if (-not $NoBundle) {
            Invoke-Checked "powershell" @(
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                ".\packaging\package-portable.ps1"
            )
        }
    }
    "linux" {
        Assert-Host "linux" "Linux AppImage/deb must be built on Linux with GTK/WebKit system libraries."
        Assert-Command "pkg-config" "Install the Tauri Linux dependencies, including GTK and WebKitGTK dev packages."
        Invoke-Checked $Npm @("run", "tauri", "build")
    }
    "macos" {
        Assert-Host "macos" "macOS app/dmg must be built on macOS with Xcode."
        Assert-Command "xcodebuild" "Install Xcode and accept the license."
        Invoke-Checked "rustup" @("target", "add", "aarch64-apple-darwin", "x86_64-apple-darwin")
        Invoke-Checked $Npm @("run", "tauri", "build", "--", "--target", "universal-apple-darwin")
    }
    "android" {
        Assert-TauriSubcommand "android"
        Ensure-Android
        if (-not (Test-Path -LiteralPath ".\src-tauri\gen\android")) {
            if (-not $InitMobile) {
                throw "Android project is not initialized. Re-run with -InitMobile after Android SDK/NDK are installed."
            }
            Invoke-Checked $Npm @("run", "tauri", "--", "android", "init")
        }
        Invoke-Checked $Npm @("run", "tauri", "--", "android", "build", "--apk")
    }
    "ios" {
        Assert-TauriSubcommand "ios"
        Assert-Host "macos" "IPA/iOS builds require macOS, Xcode and Apple signing."
        Assert-Command "xcodebuild" "Install Xcode and configure an Apple Developer team."
        if (-not (Test-Path -LiteralPath ".\src-tauri\gen\apple")) {
            if (-not $InitMobile) {
                throw "iOS project is not initialized. Re-run with -InitMobile on macOS after signing is ready."
            }
            Invoke-Checked $Npm @("run", "tauri", "--", "ios", "init")
        }
        Invoke-Checked $Npm @("run", "tauri", "--", "ios", "build")
    }
}
