# Ratadroid CLI

`ratadroid` is a command-line tool for building Ratatui (TUI) applications that run natively on Android. It scaffolds Android NativeActivity projects with Rust integration, manages Gradle-based builds, and provides development workflow automation.

![](Screenshot_20251224_073141_test_template_app.jpg)

**Note**: This is an experimental tool. The Ratatui Android Runtime is functional but may have limitations. Expect some rough edges.

## Features

- **Project scaffolding**: Creates complete Android NativeActivity projects with Gradle and Rust integration
- **Bundled template**: The template is embedded in the CLI binary - no external files needed
- **Gradle builds**: Uses Gradle for Android APKs with automatic Rust library compilation via `cargo-ndk`
- **Device management**: Detects connected devices, prefers physical devices, and can start emulators automatically
- **Logcat streaming**: The `run --log` option streams colorized logcat output after launching
- **Ratatui Android Runtime**: Custom terminal emulator that renders Ratatui applications directly to Android surfaces
- **Touch and keyboard input**: Touch events mapped to Ratatui mouse events, keyboard support with international handling
- **Environment diagnostics**: The `doctor` command checks your development environment

## Installation

Build from source:

```sh
cd ratadroid_cli
cargo install --path .
```

Or run directly:
```sh
cargo run -- <command>
```

## Prerequisites

- **Android SDK** - Install via Android Studio
- **Android NDK** - Install via Android Studio SDK Manager
- **Rust toolchain** - Install via [rustup](https://rustup.rs/)
- **cargo-ndk** - Install with `cargo install cargo-ndk`

Gradle will be downloaded automatically via Gradle Wrapper.

Run `ratadroid doctor --fix` to check your environment.

## Quick Start

### 1. Create a new project

```sh
ratadroid new my-app
cd my-app
```

This creates a complete Android project with:
- Gradle build files
- Rust library with Ratatui Android Runtime
- Demo app that runs if no custom app is registered
- Android manifest and resources

### 2. Build and run

```sh
ratadroid run
```

This builds the Rust library, builds the APK, installs it on a connected device (or starts an emulator), and launches it.

### 3. Stream logs while running

```sh
ratadroid run --log
```

Streams colorized logcat output after launching the app. Press Ctrl+C to stop.

## Commands

### `ratadroid new <name> [--path <dir>]`

Scaffold a new Android NativeActivity project.

```sh
ratadroid new my-app
```

### `ratadroid build [--variant <debug|release>]`

Build the Android project using Gradle.

```sh
ratadroid build                    # Debug build
ratadroid build --variant release  # Release build
```

### `ratadroid install [--variant <debug|release>]`

Install the APK on a connected device or emulator. Prefers physical devices over emulators.

```sh
ratadroid install
```

### `ratadroid run [--variant <debug|release>] [--log]`

Build, install, and launch the app. If no device is connected, attempts to start an available emulator.

```sh
ratadroid run           # Build and run
ratadroid run --log     # Build, run, and stream logcat
```

**Note**: Emulator auto-start waits for boot completion, which can take 30-120 seconds.

### `ratadroid devices`

List all available Android devices and emulators.

```sh
ratadroid devices
```

### `ratadroid logs [--package <name>] [--lines <n>]`

Show crash logs and errors from the app.

```sh
ratadroid logs
ratadroid logs --lines 200
```

### `ratadroid doctor [--fix]`

Check your development environment and optionally fix issues.

```sh
ratadroid doctor
ratadroid doctor --fix
```

### `ratadroid serve [--port <number>] [--dir <path>]`

Serve APKs over HTTP for easy installation on devices. Auto-detects APK output directory if `dist/` doesn't exist.

```sh
ratadroid serve              # Auto-detects app/build/outputs/apk/
ratadroid serve --port 9000  # Custom port
```

## Project Structure

```
my-app/
├── app/
│   ├── build.gradle
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── java/com/ratadroid/my_app/NativeActivity.java
│       └── res/
├── ratatui-android/         # Ratatui Android runtime library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           # Public API
│       ├── backend.rs       # Ratatui backend
│       ├── rasterizer.rs    # Software rasterizer
│       ├── input.rs         # Input handling
│       └── widgets/         # Custom widgets
├── rust/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           # Main entry point
│       ├── runtime.rs       # Android runtime loop
│       └── demo.rs          # Demo app
├── build.gradle
├── settings.gradle
└── gradlew
```

## Ratatui Android Runtime

The scaffolded projects include a Ratatui Android Runtime:

- **Direct surface rendering** - Renders Ratatui cells directly to Android surfaces
- **System fonts** - Uses Android system fonts by default (RobotoMono, DroidSansMono, etc.)
- **Touch input** - Maps Android touch events to Ratatui mouse events
- **Keyboard input** - Keyboard support with international character handling
- **Orientation support** - Handles screen rotation
- **System UI padding** - Reserves space for status bar and navigation bar

### Known Limitations

- Font rendering uses system fonts - not all Unicode characters may render perfectly
- Some input edge cases exist - international keyboard support works but may need refinement
- Software rasterization is CPU-intensive - may struggle on older devices
- Soft keyboard may not appear reliably on all devices/Android versions
- Some Ratatui widgets may not work perfectly - this is a custom backend

### Customization

Edit the Rust code to customize:
- `rust/src/lib.rs` - Main app registration and setup
- `rust/src/demo.rs` - Demo app implementation
- `ratatui-android/src/` - Runtime implementation

## Development Workflow

1. Create project: `ratadroid new my-app`
2. Edit code: Modify `rust/src/` files
3. Build and test: `ratadroid run --log`
4. Iterate

## Troubleshooting

### Build fails

- Ensure you're in a project directory created with `ratadroid new`
- Check that `cargo-ndk` is installed: `cargo install cargo-ndk`
- Verify Android SDK/NDK paths with `ratadroid doctor`

### "No devices connected"

- Connect a device via USB with USB debugging enabled
- Or create an AVD in Android Studio first
- Check with `ratadroid devices`

### Release build won't install

- Use debug builds: `ratadroid run --variant debug`
- Check device settings for "Install unknown apps" permission

### App crashes

- Use `ratadroid run --log` to see crash details in real-time
- Or check `ratadroid logs` after the crash

### Emulator takes too long to start

Emulator boot can take 1-2 minutes. The tool waits for boot completion automatically. Use a physical device for faster iteration.

## Architecture Support

Builds for:
- `arm64-v8a` (64-bit ARM) - most modern phones
- `armeabi-v7a` (32-bit ARM) - older phones

Both architectures are built by default.

## License

MIT License. See `LICENSE` for details.
