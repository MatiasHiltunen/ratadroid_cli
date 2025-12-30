# ratatui-android

Android backend for [Ratatui](https://ratatui.rs/) - enables TUI applications to run on Android devices with full touch support.

## Features

- **Software Rasterizer**: Converts Ratatui's cell grid to pixels using cosmic-text for high-quality text rendering
- **Touch Input**: Maps touch events to terminal key events
- **On-Screen Keyboard**: Built-in virtual keyboard for special keys (ESC, arrows, modifiers, etc.)
- **Unicode Support**: Full emoji and CJK character rendering via cosmic-text
- **Android Native Rendering**: Optional JNI-based rendering for best emoji support

## Quick Start

```rust
use ratatui_android::{AndroidBackend, Rasterizer, ScreenLayout, AndroidConfig};
use ratatui::Terminal;

// Create configuration
let config = AndroidConfig::default();

// Create rasterizer with desired font size
let rasterizer = Rasterizer::new(config.font_size);

// Create backend with terminal dimensions
let backend = AndroidBackend::new(80, 24);

// Create Ratatui terminal
let mut terminal = Terminal::new(backend)?;

// Draw your UI as usual
terminal.draw(|frame| {
    // Your UI code here
})?;

// Render to pixel buffer (in your Android render loop)
let mut pixels = vec![0u8; width * height * 4];
rasterizer.render_to_surface(
    terminal.backend(), 
    &mut pixels, 
    stride, 
    width, 
    height
);
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `android-native-backend` | Enable Android native text rendering via JNI. Provides best emoji support on Android but requires JNI overhead. |
| `swash-backend` | Enable swash for emoji fallback rendering |
| `ab-glyph-backend` | Enable ab_glyph for text fallback rendering |

## Integration Guide

### Step 1: Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
ratatui-android = "0.1"
ratatui = { version = "0.29", default-features = false }

[target.'cfg(target_os = "android")'.dependencies]
android-activity = { version = "0.5", features = ["native-activity"] }
ndk = "0.8"
```

### Step 2: Set Up Android Project

Create an Android project with NativeActivity. Your `AndroidManifest.xml` should include:

```xml
<activity
    android:name=".NativeActivity"
    android:configChanges="orientation|keyboardHidden|screenSize"
    android:windowSoftInputMode="stateVisible|adjustResize">
    <meta-data android:name="android.app.lib_name" android:value="your_lib_name" />
    <intent-filter>
        <action android:name="android.intent.action.MAIN" />
        <category android:name="android.intent.category.LAUNCHER" />
    </intent-filter>
</activity>
```

### Step 3: Implement Main Loop

```rust
use android_activity::{AndroidApp, MainEvent, PollEvent};
use ratatui_android::{
    AndroidBackend, Rasterizer, AndroidConfig, ScreenLayout,
    DirectKeyboard, DirectKeyboardState,
};
use ratatui::Terminal;
use std::time::Duration;

#[no_mangle]
pub extern "C" fn android_main(app: AndroidApp) {
    // Initialize logging
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info)
    );

    // Create configuration
    let config = AndroidConfig::default();
    
    // Create rasterizer
    let rasterizer = Rasterizer::new(config.font_size);
    
    // Warm the character cache for better first-frame performance
    ratatui_android::warm_cache(config.font_size);
    
    // Create backend (will be resized when window is ready)
    let backend = AndroidBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("Failed to create terminal");
    
    // Create keyboard
    let direct_keyboard = DirectKeyboard::new();
    let mut keyboard_state = DirectKeyboardState::new();
    
    // Main loop
    let mut running = true;
    while running {
        app.poll_events(Some(Duration::from_millis(16)), |event| {
            match event {
                PollEvent::Main(MainEvent::InitWindow { .. }) => {
                    // Window is ready - resize backend
                    if let Some(window) = app.native_window() {
                        let layout = ScreenLayout::calculate(
                            window.width() as u32,
                            window.height() as u32,
                            window.height() as u32,
                            &config,
                            &rasterizer,
                        );
                        terminal.backend_mut().resize(layout.cols, layout.rows);
                    }
                }
                PollEvent::Main(MainEvent::Destroy) => {
                    running = false;
                }
                _ => {}
            }
        });
        
        // Draw TUI
        terminal.draw(|frame| {
            // Your UI code here
        }).ok();
        
        // Render to window
        if let Some(window) = app.native_window() {
            // Lock the window buffer and render
            // (Implementation depends on your window management approach)
        }
    }
}
```

### Step 4: Handle Touch Input

```rust
use ratatui_android::input::{TouchEvent, TouchAction, key_to_crossterm_event};
use android_activity::input::InputEvent;

fn handle_input(
    input_event: &InputEvent, 
    terminal: &mut Terminal<AndroidBackend>,
    keyboard: &DirectKeyboard,
    keyboard_state: &mut DirectKeyboardState,
    layout: &ScreenLayout,
    config: &AndroidConfig,
) {
    if let InputEvent::MotionEvent(motion) = input_event {
        use android_activity::input::MotionAction;
        
        if motion.action() == MotionAction::Down {
            if let Some(pointer) = motion.pointers().next() {
                let touch_x = pointer.x() as usize;
                let touch_y = pointer.y() as usize;
                
                // Check if touch is in keyboard area
                let keyboard_y = layout.keyboard_y(
                    config.nav_bar_height, 
                    config.keyboard_height
                );
                
                if touch_y >= keyboard_y {
                    // Handle keyboard touch
                    if let Some(key_name) = keyboard.handle_touch(
                        touch_x,
                        touch_y,
                        layout.width_px as usize,
                        keyboard_y,
                        (config.keyboard_height / 2).saturating_sub(4).max(20),
                    ) {
                        keyboard_state.set_pressed(key_name.to_string());
                        
                        // Handle modifier toggles
                        match key_name {
                            "SHIFT" => keyboard_state.toggle_shift(),
                            "CTRL" => keyboard_state.toggle_ctrl(),
                            _ => {}
                        }
                        
                        // Convert to crossterm event
                        if let Some(event) = key_to_crossterm_event(
                            key_name,
                            keyboard_state.shift_active,
                            keyboard_state.ctrl_active,
                        ) {
                            // Handle the event in your TUI
                        }
                    }
                } else {
                    // Handle touch in TUI area
                    let col = (touch_x as f32 / layout.font_width) as u16;
                    let row = ((touch_y - layout.top_offset_rows as usize) as f32 
                        / layout.font_height) as u16;
                    // Handle TUI touch...
                }
            }
        }
    }
}
```

### Step 5: Java-Side Setup (Optional but Recommended)

For proper keyboard visibility handling, create a Java `NativeActivity`:

```java
package com.example.myapp;

import android.os.Bundle;
import android.view.View;
import android.view.ViewTreeObserver;
import android.graphics.Rect;

public class NativeActivity extends android.app.NativeActivity {
    static {
        System.loadLibrary("your_lib_name");
    }
    
    private int mLastVisibleHeight = -1;
    private boolean mKeyboardVisible = false;
    
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        
        // Monitor keyboard visibility
        final View rootView = getWindow().getDecorView().getRootView();
        rootView.getViewTreeObserver().addOnGlobalLayoutListener(
            new ViewTreeObserver.OnGlobalLayoutListener() {
                @Override
                public void onGlobalLayout() {
                    Rect r = new Rect();
                    rootView.getWindowVisibleDisplayFrame(r);
                    int visibleHeight = r.height();
                    
                    if (mLastVisibleHeight != -1 && 
                        Math.abs(visibleHeight - mLastVisibleHeight) > 100) {
                        int screenHeight = rootView.getHeight();
                        mKeyboardVisible = (screenHeight - visibleHeight) > (screenHeight * 0.15);
                        notifyKeyboardVisibilityChanged(mKeyboardVisible, visibleHeight);
                    }
                    mLastVisibleHeight = visibleHeight;
                }
            });
    }
    
    // JNI callback to Rust
    private native void notifyKeyboardVisibilityChanged(boolean visible, int visibleHeight);
    
    // Called from Rust to show soft keyboard
    public void showSoftKeyboard() {
        android.view.inputmethod.InputMethodManager imm = 
            (android.view.inputmethod.InputMethodManager) 
            getSystemService(android.content.Context.INPUT_METHOD_SERVICE);
        if (imm != null) {
            View view = getWindow().getDecorView();
            imm.showSoftInput(view, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT);
        }
    }
    
    // Get navigation bar height
    public int getNavigationBarHeight() {
        int resourceId = getResources().getIdentifier("navigation_bar_height", "dimen", "android");
        if (resourceId > 0) {
            return getResources().getDimensionPixelSize(resourceId);
        }
        return 0;
    }
}
```

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    Your TUI Application                   │
├──────────────────────────────────────────────────────────┤
│                        Ratatui                           │
│                   (Terminal, Frame, etc.)                │
├──────────────────────────────────────────────────────────┤
│                   ratatui-android                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │AndroidBackend│  │ Rasterizer  │  │ DirectKeyboard  │  │
│  │(cell buffer)│  │(cosmic-text)│  │(pixel renderer) │  │
│  └─────────────┘  └─────────────┘  └─────────────────┘  │
├──────────────────────────────────────────────────────────┤
│                  Android Native Window                   │
│                   (ANativeWindow)                        │
└──────────────────────────────────────────────────────────┘
```

## Performance Tips

1. **Warm the cache**: Call `warm_cache(font_size)` at startup to pre-render common characters.

2. **Minimize redraws**: Only call `terminal.draw()` when the UI actually changed.

3. **Use the DirectKeyboard**: It's faster than the Ratatui-based KeyboardWidget for keyboard rendering.

4. **Batch touch events**: Process touch events in batches rather than one at a time.

## License

MIT - Same as Ratatui

