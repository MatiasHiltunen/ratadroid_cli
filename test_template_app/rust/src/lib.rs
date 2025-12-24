//! Ratatui Android Runtime - A complete terminal emulator for Android NativeActivity
//!
//! This implementation acts as a terminal emulator, rendering Ratatui's cell grid
//! directly to an Android Surface using software rasterization.

#[cfg(target_os = "android")]
mod backend;
#[cfg(target_os = "android")]
mod rasterizer;
#[cfg(target_os = "android")]
mod input;
#[cfg(target_os = "android")]
mod button;
#[cfg(target_os = "android")]
mod todo_app;

#[cfg(target_os = "android")]
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
#[cfg(target_os = "android")]
use android_logger::Config;
#[cfg(target_os = "android")]
use log::{info, warn};
#[cfg(target_os = "android")]
use ndk::native_window::NativeWindow;
#[cfg(target_os = "android")]
use ndk_sys::ANativeWindow_setBuffersGeometry;
#[cfg(target_os = "android")]
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
#[cfg(target_os = "android")]
use std::time::Duration;

// Embed a monospace font
// Place Hack-Regular.ttf in rust/fonts/ directory
const FONT_DATA: &[u8] = include_bytes!("../fonts/Hack-Regular.ttf");
const FONT_SIZE: f32 = 36.0; // Larger text for mobile screens

/// Application state structure
#[cfg(target_os = "android")]
struct AppState {
    terminal: Terminal<backend::AndroidBackend>,
    rasterizer: rasterizer::Rasterizer<'static>,
    should_quit: bool,
    native_window: Option<NativeWindow>,
    todo_app: todo_app::TodoApp,
}

/// Android NativeActivity entry point
/// android-activity crate with "native-activity" feature bridges ANativeActivity_onCreate
/// to this function automatically
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: AndroidApp) {
    android_logger::init_once(Config::default().with_max_level(log::LevelFilter::Info));
    info!("Ratatui Android Runtime starting");

    // Initialize Rasterizer with embedded font
    let rasterizer = match rasterizer::Rasterizer::new(FONT_DATA, FONT_SIZE) {
        Ok(r) => {
            info!("Font loaded successfully");
            r
        }
        Err(e) => {
            warn!("Failed to load font, using fallback: {}", e);
            warn!("To fix: Download Hack-Regular.ttf and place in rust/fonts/ directory");
            rasterizer::Rasterizer::new_fallback(FONT_SIZE)
        }
    };
    
    // Initialize Ratatui backend with dummy size (will resize on window init)
    let backend = backend::AndroidBackend::new(1, 1);
    let mut input_state = input::InputState::new();
    let mut state = AppState {
        terminal: Terminal::new(backend).unwrap(),
        rasterizer,
        should_quit: false,
        native_window: None,
        todo_app: todo_app::TodoApp::new(),
    };

    // Main event loop
    loop {
        // Poll input events first - this prevents ANR (Application Not Responding)
        if let Ok(mut input_iter) = app.input_events_iter() {
            while input_iter.next(|input_event| {
                // Map Android Input -> TUI Event
                match input::map_android_event(
                    input_event,
                    &app,
                    &mut input_state,
                    state.rasterizer.font_width(),
                    state.rasterizer.font_height(),
                ) {
                    Some(tui_event) => {
                        // Handle mouse clicks on buttons
                        let mut button_clicked = false;
                        if let crossterm::event::Event::Mouse(crossterm::event::MouseEvent {
                            kind: crossterm::event::MouseEventKind::Down(_),
                            column,
                            row,
                            ..
                        }) = &tui_event {
                            // Get the terminal size for button click detection
                            if let Ok(term_size) = state.terminal.size() {
                                button_clicked = state.todo_app.handle_mouse_click(*column, *row, term_size);
                            }
                        }
                        
                        // Handle other events (including quit) if button wasn't clicked
                        if !button_clicked {
                            if state.todo_app.handle_event(&tui_event) {
                                state.should_quit = true;
                            }
                        }
                    }
                    None => {
                        // Event not mapped (e.g., unsupported key or motion action)
                        // This is fine, we just ignore it
                    }
                }
                // Return Handled to acknowledge we processed the event (prevents ANR)
                InputStatus::Handled
            }) {
                // Continue processing events
            }
        }
        
        app.poll_events(Some(Duration::from_millis(16)), |event| {
            match event {
                PollEvent::Main(main_event) => {
                    handle_lifecycle(&mut state, &app, main_event);
                }
                PollEvent::Wake => {
                    // Triggered if we need to wake up for rendering
                }
                _ => {}
            }
        });

        if state.should_quit {
            break;
        }

        // Render if we have a valid window
        // Clone the window reference to avoid borrow checker issues
        let window = state.native_window.clone();
        if let Some(w) = window {
            draw_tui(&mut state, &w);
        }
    }
    
    info!("Ratatui Android Runtime exiting");
}

#[cfg(target_os = "android")]
fn handle_lifecycle(state: &mut AppState, app: &AndroidApp, event: MainEvent) {
    match event {
        MainEvent::InitWindow { .. } => {
            info!("Window initialized");
            // Window is ready. We can now lock it to draw.
            if let Some(win) = app.native_window().as_ref() {
                // Explicitly set buffer format to RGBA_8888
                // This ensures the buffer format matches what we expect
                // WINDOW_FORMAT_RGBA_8888 = 1 (from android/native_window.h)
                let width = win.width();
                let height = win.height();
                unsafe {
                    let native_window_ptr = win.ptr().as_ptr() as *mut ndk_sys::ANativeWindow;
                    // WINDOW_FORMAT_RGBA_8888 = 1
                    let result = ANativeWindow_setBuffersGeometry(
                        native_window_ptr,
                        width as i32,
                        height as i32,
                        1, // WINDOW_FORMAT_RGBA_8888
                    );
                    if result == 0 {
                        info!("Successfully set buffer format to RGBA_8888 ({}x{})", width, height);
                    } else {
                        warn!("Failed to set buffer format, result code: {}", result);
                    }
                }
                
                state.native_window = Some(win.clone());
                // Resize backend to match window
                resize_backend(state, win);
            }
        }
        MainEvent::WindowResized { .. } | MainEvent::ConfigChanged { .. } => {
            info!("Window resized or config changed");
            // Re-measure grid - clone window to avoid borrow checker issues
            let window = state.native_window.clone();
            if let Some(w) = window {
                resize_backend(state, &w);
            }
        }
        MainEvent::Destroy => {
            info!("Activity destroyed, quitting");
            state.should_quit = true;
        }
        _ => {}
    }
}

#[cfg(target_os = "android")]
fn resize_backend(state: &mut AppState, window: &NativeWindow) {
    let width_px = window.width() as f32;
    let height_px = window.height() as f32;
    
    // Calculate how many characters fit
    let cols = (width_px / state.rasterizer.font_width()) as u16;
    let rows = (height_px / state.rasterizer.font_height()) as u16;
    
    if cols > 0 && rows > 0 {
        state.terminal.backend_mut().resize(cols, rows);
        info!("Resized terminal to {}x{} (window: {}x{})", cols, rows, width_px, height_px);
        // Force a full redraw
        let _ = state.terminal.clear();
    }
}

#[cfg(target_os = "android")]
fn draw_tui(state: &mut AppState, window: &NativeWindow) {
    // A. Ratatui Render Pass
    // This updates the internal Cell buffer, doesn't draw pixels yet.
    let _ = state.terminal.draw(|frame| {
        let area = frame.size();
        
        // Render the todo app with buttons
        state.todo_app.render_frame(frame, area);
        
        // Clear button clicked state after rendering (for visual feedback)
        // This creates a brief flash effect when button is clicked
        if state.todo_app.button_clicked.is_some() {
            state.todo_app.button_clicked = None;
        }
    });

    // B. Pixel Blit Pass
    // Lock the Android hardware buffer and render pixels
    match window.lock(None) {
        Ok(mut buffer) => {
            let stride = buffer.stride() as usize;
            let height = buffer.height() as usize;
            
            // Get mutable slice to pixel data (RGBA format)
            // buffer.bits() returns *mut c_void, we need to convert it to a slice
            let bits_ptr = buffer.bits();
            if !bits_ptr.is_null() {
                let window_width = window.width() as usize;
                let window_height = window.height() as usize;
                
                // Safety check: ensure buffer dimensions match window dimensions
                // Use the minimum of each to ensure we don't access out of bounds
                let safe_height = height.min(window_height);
                let buffer_width = buffer.width() as usize;
                let safe_width = buffer_width.min(window_width);
                
                // Validate safe dimensions
                if safe_height == 0 || safe_width == 0 {
                    warn!("Invalid safe dimensions: {}x{}", safe_width, safe_height);
                    return;
                }
                
                // Use stride for buffer size calculation (it includes padding)
                // Calculate maximum safe buffer size
                let max_buffer_size_bytes = match stride.checked_mul(safe_height)
                    .and_then(|pixels| pixels.checked_mul(4))
                {
                    Some(size) => size,
                    None => {
                        warn!("Buffer size calculation overflow: stride={}, height={}", stride, safe_height);
                        return;
                    }
                };
                
                // Create mutable slice (unsafe but necessary for pixel manipulation)
                // Use the calculated safe buffer size
                let pixels_mut = unsafe {
                    std::slice::from_raw_parts_mut(bits_ptr as *mut u8, max_buffer_size_bytes)
                };
                
                // Rasterize cells to pixels
                // Pass both stride (for buffer layout) and safe_width (for bounds checking)
                state.rasterizer.render_to_surface(
                    state.terminal.backend(),
                    pixels_mut,
                    stride,         // Use stride for buffer layout
                    safe_width,     // Use safe_width for bounds checking
                    safe_height,    // Use safe_height for bounds checking
                );
            }
            
            // Unlock and post to screen
            // The buffer guard will unlock when dropped, then we post
            drop(buffer);
            // Note: In ndk 0.8, unlock_and_post is called automatically on drop
            // If we need explicit posting, we might need to use ANativeWindow_post from ndk-sys
        }
        Err(e) => {
            warn!("Failed to lock window buffer: {:?}", e);
        }
    }
}

/// Standalone entry point for testing on desktop
#[cfg(not(target_os = "android"))]
fn main() {
    println!("This is an Android-only library. Use 'cargo build --target aarch64-linux-android' to build for Android.");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        // Basic test to ensure the module compiles
        assert!(true);
    }
}
