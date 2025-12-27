# Ratadroid Template

A template for building Android TUI applications using [Ratatui](https://ratatui.rs/).

## Overview

This template provides everything needed to run a Ratatui-based TUI application on Android:

- **ratatui-android crate**: Ratatui backend for Android with software rasterization
- **rust/** - Template Rust library with the `RatadroidApp` trait
- **app/** - Android app with NativeActivity for touch input, keyboard handling, and rendering

## Quick Start

### 1. Create Your App

Implement the `RatadroidApp` trait:

```rust
use ratadroid::{RatadroidApp, RatadroidContext, set_app_factory};
use ratatui::prelude::*;
use crossterm::event::{Event, KeyCode};

struct MyApp {
    // Your app state
}

impl RatadroidApp for MyApp {
    fn name(&self) -> &str {
        "My App"
    }

    fn draw(&mut self, frame: &mut ratatui::Frame, ctx: &RatadroidContext) {
        // Draw your UI using Ratatui widgets
    }

    fn handle_event(&mut self, event: Event, ctx: &mut RatadroidContext) -> bool {
        // Handle input events
        if let Event::Key(key) = event {
            if key.code == KeyCode::Esc {
                ctx.quit();
                return true;
            }
        }
        false
    }
}

// Register your app factory
fn register_my_app() {
    set_app_factory(|| Box::new(MyApp { /* ... */ }));
}
```

### 2. Register Your App

The app factory must be registered before `android_main` runs. The typical pattern is:

```rust
// In your lib.rs
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: android_activity::AndroidApp) {
    // Register your app
    register_my_app();
    
    // Run the ratadroid runtime
    ratadroid::android_main(app);
}
```

### 3. Build for Android

```bash
cd rust
cargo ndk -t arm64-v8a -t armeabi-v7a build --release
cd ../app
./gradlew assembleRelease
```

The APK will be in `app/build/outputs/apk/release/`.

## Project Structure

```
ratadroid_template/
├── ratatui-android/         # Ratatui Android backend crate
│   ├── src/
│   │   ├── lib.rs          # Crate entry point
│   │   ├── backend.rs      # Ratatui Backend implementation
│   │   ├── rasterizer.rs   # Text rendering with cosmic-text
│   │   ├── android_render.rs # JNI rendering (feature-gated)
│   │   ├── widgets/        # On-screen keyboard widgets
│   │   └── input.rs        # Input conversion utilities
│   └── Cargo.toml
├── rust/                    # Template runtime
│   ├── src/
│   │   ├── lib.rs          # RatadroidApp trait and public API
│   │   ├── runtime.rs      # Android lifecycle and event loop
│   │   └── demo.rs         # Example demo app
│   └── Cargo.toml
├── app/                     # Android app
│   ├── src/main/
│   │   ├── AndroidManifest.xml
│   │   └── java/.../NativeActivity.java
│   └── build.gradle
└── README.md
```

## RatadroidApp Trait

```rust
pub trait RatadroidApp: Send {
    /// App name for logging
    fn name(&self) -> &str { "RatadroidApp" }

    /// Initialize the app (called once after Android context is ready)
    fn init(&mut self, ctx: &RatadroidContext) -> anyhow::Result<()> { Ok(()) }

    /// Draw the app's UI (called each frame)
    fn draw(&mut self, frame: &mut ratatui::Frame, ctx: &RatadroidContext);

    /// Handle input events (return true if consumed)
    fn handle_event(&mut self, event: Event, ctx: &mut RatadroidContext) -> bool;

    /// Called on screen resize
    fn on_resize(&mut self, cols: u16, rows: u16, ctx: &RatadroidContext) {}

    /// Called periodically (for async operations)
    fn tick(&mut self, ctx: &mut RatadroidContext) {}
}
```

## RatadroidContext

```rust
pub struct RatadroidContext {
    pub should_quit: bool,      // Set to true to exit
    pub needs_draw: bool,       // Set to true to request redraw
    pub data_dir: PathBuf,      // Android app data directory
    pub orientation: Orientation, // Portrait or Landscape
    pub cols: u16,              // Terminal columns
    pub rows: u16,              // Terminal rows
    pub font_width: f32,        // Font cell width in pixels
    pub font_height: f32,       // Font cell height in pixels
}
```

## Features

- **Full Unicode Support**: Emojis, CJK characters, and all Unicode rendered via cosmic-text
- **On-Screen Keyboard**: Built-in special key keyboard (ESC, arrows, Tab, etc.)
- **Touch Input**: Touch events converted to terminal mouse events
- **Soft Keyboard**: Android soft keyboard integration
- **Screen Orientation**: Automatic handling of rotation

## Customizing

### Package Name

To use a different package name:

1. Update `app/build.gradle`: `namespace` and `applicationId`
2. Move Java files to matching package directory
3. Update `AndroidManifest.xml` if needed
4. Update the JNI callback function name in `runtime.rs`

### Native Library Name

To use a different library name:

1. Update `rust/Cargo.toml`: `[lib] name = "your_name"`
2. Update `app/build.gradle`: change `libratadroid.so` references
3. Update `NativeActivity.java`: `System.loadLibrary("your_name")`
4. Update `AndroidManifest.xml`: `android:value="your_name"`

## License

MIT

