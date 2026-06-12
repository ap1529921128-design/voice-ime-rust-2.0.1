# Voice IME 跨平台打包说明

## 当前结论

Windows 2.0.1 已经进入可发布状态。Mac、Linux、Android、iOS 是新的跨平台阶段，不是把 Windows 包换个后缀就能完成。

当前代码已经先完成第一层跨平台地基：

- Windows 继续使用完整输入目标能力：UI Automation / GUI thread 光标定位、恢复目标窗口、剪贴板粘贴、必要时短文本直接输入。
- 非 Windows 先走保守 fallback：主窗口和转写链路可复用，确认输入改为复制到剪贴板，日志标记 `clipboard-only-fallback`。
- Tauri 平台配置已拆分为 `tauri.windows.conf.json`、`tauri.macos.conf.json`、`tauri.linux.conf.json`、`tauri.android.conf.json`、`tauri.ios.conf.json`。
- Windows-only Rust 依赖已经挪到 Windows target 下，后续 Mac/Linux 不会再被 Win32 API 直接卡住。

## 平台目标

| 平台 | 第一版可落地形态 | 系统输入法/光标粘贴状态 |
| --- | --- | --- |
| Windows | 已完成：桌面语音输入、浮窗、确认粘贴、模型分离 | 已实现 |
| macOS | 桌面 App：录音、转写、纠错/翻译、复制到剪贴板 | 后续接 Accessibility / CGEvent |
| Linux | 桌面 App：录音、转写、纠错/翻译、复制到剪贴板 | 后续区分 X11 / Wayland |
| Android | APK App：录音、转写、复制/分享文本 | 真正系统键盘需单独做 IME service |
| iOS | IPA App：录音、转写、复制/分享文本 | 键盘扩展受系统限制，需后续单独设计 |

## 一键入口

项目根目录运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\Build-CrossPlatform.ps1 -Platform windows
```

可选平台：

```powershell
# Windows：只应在 Windows 上执行，产出 Tauri NSIS 包和 portable 包
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\Build-CrossPlatform.ps1 -Platform windows

# Linux：只应在 Linux 上执行，产出 AppImage/deb
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform linux

# macOS：只应在 macOS + Xcode 上执行，产出 universal app/dmg
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform macos

# Android：可在装好 Android Studio/SDK/NDK/JDK 的机器上执行，产出 APK
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform android -InitMobile

# iOS：只能在 macOS + Xcode + Apple Developer 签名环境执行
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform ios -InitMobile
```

`-InitMobile` 只在第一次生成 Android/iOS 项目骨架时使用。已经生成过 `src-tauri/gen/android` 或 `src-tauri/gen/apple` 后，正常打包不需要加。

## Host 要求

### Windows

- Rust stable MSVC
- Node.js / npm
- Tauri CLI 由本项目 `node_modules` 提供

当前公开发布仍推荐 core 包：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-release-assets.ps1
```

### Linux

Linux 包必须在 Linux 上构建。Windows 交叉检查会卡在 GTK/WebKitGTK 的 `pkg-config` sysroot，这是正常的。

典型依赖：

- Rust stable
- Node.js / npm
- `pkg-config`
- GTK / WebKitGTK / appindicator 相关开发包
- 系统音频输入权限和设备

执行：

```bash
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform linux
```

### macOS

macOS 包必须在 macOS 上构建。Windows 没有 Apple clang、Objective-C runtime 和 Xcode SDK，无法生成 `.app` / `.dmg`。

要求：

- macOS
- Xcode / Command Line Tools
- Rust targets：`aarch64-apple-darwin`、`x86_64-apple-darwin`
- 如需正式分发，还需要 Developer ID 签名和 notarization

执行：

```bash
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform macos
```

### Android

第一版 APK 目标是“手机语音转文字工具”，不是系统级输入法。

要求：

- JDK 17+
- Android Studio
- Android SDK / NDK
- `ANDROID_HOME` 或 `ANDROID_SDK_ROOT`
- Rust Android targets 由 Tauri mobile 初始化流程补齐

执行：

```bash
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform android -InitMobile
```

后续要做真正系统键盘，需要新增 Android IME service，并把 Rust/Tauri 主体拆成可被键盘服务调用的语音识别核心。

### iOS

IPA 只能在 macOS + Xcode + Apple 签名环境中完成。第一版目标是 iOS App，系统键盘扩展另做。
当前 Windows 本机的 Tauri CLI 没有暴露 `ios` 子命令；脚本会在目标 Mac 上先检查 CLI 能力，不具备时提示升级或按 Tauri 当前 mobile 文档重新初始化。

要求：

- macOS
- Xcode
- Apple Developer Team
- 可用的 Bundle Identifier、Provisioning Profile、Signing Certificate

执行：

```bash
pwsh ./packaging/Build-CrossPlatform.ps1 -Platform ios -InitMobile
```

## 本机验证记录

在当前 Windows 机器上已验证：

```text
cargo test
134 passed
```

已做交叉体检：

- `cargo check --target x86_64-unknown-linux-gnu`：卡在 Linux GTK/WebKit `pkg-config` sysroot，属于 Windows 交叉环境缺失。
- `cargo check --target x86_64-apple-darwin`：卡在 Apple Objective-C/macOS 编译工具链，属于 Windows 主机无法提供 Xcode SDK。

下一步应在真实 Linux/macOS runner 上继续验证业务代码编译，并把平台原生输入目标补齐。
