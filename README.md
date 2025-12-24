# Ratadroid CLI

`ratadroid` is a command-line tool for building Ratatui (TUI) applications that run natively on Android. It scaffolds Android NativeActivity projects with Rust integration, manages Gradle-based builds, and provides basic development workflow automation.

![](Screenshot_20251224_073141_test_template_app.jpg)

**Note**: This is an experimental tool. The Ratatui Android Runtime is functional but may have limitations and edge cases. Expect some rough edges.

## Features

- **Project scaffolding**: The `new` command creates an Android NativeActivity project with Gradle build configuration and Rust integration
- **Gradle builds**: Uses Gradle to build Android APKs with automatic Rust library compilation via `cargo-ndk`
- **Device management**: Detects connected devices and can start emulators if none are available
- **Ratatui Android Runtime**: A custom terminal emulator implementation that renders Ratatui applications directly to Android surfaces
- **Basic input support**: Touch input and keyboard support (including some international keyboard handling)
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
- **Android NDK** - Install via Android Studio SDK Manager (version 25.1.8937393)
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

This creates a project with:
- Gradle build files
- Rust library with Ratatui Android Runtime
- Example todo app
- Basic Android manifest

### 2. Build and run

```sh
ratadroid run
```

This builds the Rust library, builds the APK, and installs it on a connected device (or starts an emulator if none available).

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
ratadroid build --variant release  # Release build (signed with debug keystore)
```

### `ratadroid install [--variant <debug|release>]`

Install the APK on a connected device or emulator. Prefers physical devices over emulators.

```sh
ratadroid install
```

### `ratadroid run [--variant <debug|release>]`

Build, install, and launch the app. If no device is connected, attempts to start an available emulator.

```sh
ratadroid run
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

Serve APKs over HTTP for easy installation on devices.

```sh
ratadroid serve
```

## Project Structure

```
my-app/
├── app/
│   ├── build.gradle
│   └── src/main/
│       ├── AndroidManifest.xml
│       ├── java/com/ratadroid/my_app/NativeActivity.java
│       └── res/values/strings.xml
├── rust/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs           # Main entry point
│   │   ├── backend.rs       # Ratatui backend
│   │   ├── rasterizer.rs    # Software rasterizer
│   │   └── input.rs         # Input handling
│   └── fonts/               # Optional font files
├── build.gradle
├── settings.gradle
└── gradlew
```

## Ratatui Android Runtime

The scaffolded projects include a Ratatui Android Runtime implementation:

- **Direct surface rendering** - Renders Ratatui cells directly to Android surfaces
- **Software rasterization** - Uses embedded fonts to convert cells to pixels
- **Touch input** - Maps Android touch events to Ratatui mouse events
- **Keyboard input** - Basic keyboard support with some international character handling
- **Orientation support** - Handles screen rotation
- **System UI padding** - Reserves space for status bar and navigation bar

### Known Limitations

- Font rendering is basic - uses embedded TTF fonts, may not handle all Unicode characters perfectly
- Input handling has some edge cases - Scandinavian keyboard support works but may need refinement
- Performance - Software rasterization is CPU-intensive, may struggle on older devices
- Soft keyboard - May not appear reliably on all devices/Android versions
- Some Ratatui widgets may not work perfectly - the runtime is a custom backend implementation

### Customization

Edit the Rust code in `rust/src/` to customize:
- `lib.rs` - Main app logic and event loop
- `backend.rs` - Ratatui backend implementation
- `rasterizer.rs` - Rendering logic
- `input.rs` - Input event handling

## Development Workflow

1. Create project: `ratadroid new my-app`
2. Edit code: Modify `rust/src/lib.rs` and other Rust files
3. Build and test: `ratadroid run`
4. View logs: `ratadroid logs` (in another terminal)
5. Iterate

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

Release builds are signed with a debug keystore. If installation fails:
- Use debug builds: `ratadroid run --variant debug`
- Check device settings for "Install unknown apps" permission
- Some devices may reject unsigned release APKs even with `-t` flag

### Keyboard not showing

The soft keyboard implementation uses JNI and may not work reliably on all devices:
- Check `ratadroid logs` for errors
- Try tapping the text input area multiple times
- Physical keyboards work more reliably

### App crashes or renders incorrectly

- Check `ratadroid logs` for crash details
- Verify font file exists in `rust/fonts/Hack-Regular.ttf`
- Try reducing font size in `rust/src/lib.rs` (FONT_SIZE constant)
- Some Ratatui widgets may not render correctly - this is experimental

### Emulator takes too long to start

Emulator boot can take 1-2 minutes. The tool waits for boot completion automatically. Be patient or use a physical device for faster iteration.

## Architecture Support

Builds for:
- `arm64-v8a` (64-bit ARM)
- `armeabi-v7a` (32-bit ARM)
- `x86_64` (64-bit x86)
- `x86` (32-bit x86)

All architectures are built by default. The `--target` option for `build` is not yet implemented (builds all architectures).

## Limitations and Future Work

- Some Ratatui features may not work perfectly
- Performance optimization needed for complex UIs
- Better font fallback handling
- More robust keyboard support
- Better error messages and diagnostics

## Contributing

This is an experimental project. Contributions, bug reports, and feedback are welcome. Expect rough edges and incomplete features.

## License

MIT License. See `LICENSE` for details.
