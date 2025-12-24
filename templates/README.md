# {name}

Android NativeActivity application with Rust and Ratatui integration.

## Building

```bash
# Build debug APK
ratadroid build --variant debug

# Build release APK
ratadroid build --variant release
```

## Installing

```bash
# Install debug APK on connected device/emulator
ratadroid install --variant debug
```

## Running

```bash
# Build, install, and run the app
ratadroid run --variant debug
```

## Development

The Rust code is located in the `rust/` directory. After modifying Rust code, rebuild:

```bash
ratadroid build
```

## Project Structure

- `app/` - Android application module
- `rust/` - Rust library with Ratatui TUI
- `build.gradle` - Root Gradle build file
- `settings.gradle` - Gradle settings

## Requirements

- Android SDK with NDK
- Gradle 8.5+
- Rust toolchain with Android targets
- cargo-ndk (for legacy builds)

