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
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
#[cfg(target_os = "android")]
use android_logger::Config;
#[cfg(target_os = "android")]
use log::{info, warn};
#[cfg(target_os = "android")]
use ndk::native_window::NativeWindow;
#[cfg(target_os = "android")]
use ndk_sys::{ANativeWindow_setBuffersGeometry, ANativeActivity};
#[cfg(target_os = "android")]
use jni::{JNIEnv, objects::JObject};
#[cfg(target_os = "android")]
use jni::sys::{jint, jobject};
#[cfg(target_os = "android")]
use ndk::native_activity::NativeActivity;
#[cfg(target_os = "android")]
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
#[cfg(target_os = "android")]
use std::time::Duration;

// Embed a monospace font
// Place Hack-Regular.ttf in rust/fonts/ directory
const FONT_DATA: &[u8] = include_bytes!("../fonts/Hack-Regular.ttf");
const FONT_SIZE: f32 = 36.0; // Larger text for mobile screens

/// Screen orientation
#[cfg(target_os = "android")]
#[derive(Debug, Clone, Copy, PartialEq)]
enum Orientation {
    Portrait,
    Landscape,
}

/// Application state structure
#[cfg(target_os = "android")]
struct AppState {
    terminal: Terminal<backend::AndroidBackend>,
    rasterizer: rasterizer::Rasterizer<'static>,
    should_quit: bool,
    native_window: Option<NativeWindow>,
    top_offset_rows: u16, // Number of rows to skip at top for status bar
    bottom_offset_rows: u16, // Number of rows to skip at bottom for navigation bar
    orientation: Orientation, // Current screen orientation
}

/// Android NativeActivity entry point
/// android-activity crate with "native-activity" feature bridges ANativeActivity_onCreate
/// to this function automatically
#[cfg(target_os = "android")]
#[no_mangle]
#[allow(improper_ctypes_definitions)]
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
        top_offset_rows: 2, // Default: skip 2 rows at top for status bar
        bottom_offset_rows: 2, // Default: skip 2 rows at bottom for navigation bar
        orientation: Orientation::Portrait, // Default orientation
    };

    // Main event loop
    loop {
        // Poll input events first - this prevents ANR (Application Not Responding)
        if let Ok(mut input_iter) = app.input_events_iter() {
            // Get window height for input coordinate adjustment
            let window_height = if let Some(ref window) = state.native_window {
                window.height() as f32
            } else {
                0.0
            };
            
            while input_iter.next(|input_event| {
                // Calculate offsets in pixels for input coordinate adjustment
                let top_offset_px = state.top_offset_rows as f32 * state.rasterizer.font_height();
                let bottom_offset_px = state.bottom_offset_rows as f32 * state.rasterizer.font_height();
                
                // Map Android Input -> TUI Event
                match input::map_android_event(
                    input_event,
                    &app,
                    &mut input_state,
                    state.rasterizer.font_width(),
                    state.rasterizer.font_height(),
                    top_offset_px,
                    bottom_offset_px,
                    window_height,
                ) {
                    Some(tui_event) => {
                        // Handle the event - you can add your own event handling logic here
                        // For example, check for quit key, update app state, etc.
                        // Example: if matches!(tui_event, Event::Key(KeyEvent { code: KeyCode::Char('q'), .. })) {
                        //     state.should_quit = true;
                        // }
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

/// Show the Android soft keyboard by calling Java method via JNI
/// Uses ndk-glue to access ANativeActivity
#[cfg(target_os = "android")]
fn show_soft_keyboard(_app: &AndroidApp) {
    info!("show_soft_keyboard() called - attempting to show keyboard");
    
    unsafe {
        // Use ndk-glue to get NativeActivity
        // Note: ndk-glue should be initialized by android-activity
        let native_activity = ndk_glue::native_activity();
        
        // Get Java VM from NativeActivity
        let vm = native_activity.vm();
        if vm.is_null() {
            warn!("Java VM is null from ndk-glue");
            return;
        }
        info!("Got Java VM from ndk-glue");
        
        // Get Activity object (Java object) from NativeActivity
        let activity_obj = native_activity.activity();
        if activity_obj.is_null() {
            warn!("Activity object is null from ndk-glue");
            return;
        }
        info!("Got Activity object from ndk-glue");
        
        // Attach to Java VM
        let vm = match jni::JavaVM::from_raw(vm as *mut _) {
            Ok(vm) => vm,
            Err(e) => {
                warn!("Failed to create JavaVM: {:?}", e);
                return;
            }
        };
        info!("Created JavaVM from raw pointer");
        
        let mut env = match vm.attach_current_thread_permanently() {
            Ok(env) => env,
            Err(e) => {
                warn!("Failed to attach to Java VM: {:?}", e);
                return;
            }
        };
        info!("Attached to Java VM thread");
        
        let activity_jobj = JObject::from_raw(activity_obj as jobject);
        info!("Created JObject from activity pointer");
        
        // Call showSoftKeyboard() method on NativeActivity
        info!("Calling showSoftKeyboard() method");
        match env.call_method(
            activity_jobj,
            "showSoftKeyboard",
            "()V",
            &[]
        ) {
            Ok(_) => {
                info!("Soft keyboard shown successfully via Java method");
            }
            Err(e) => {
                warn!("Failed to call showSoftKeyboard: {:?}", e);
                // Try to get more details about the error
                if env.exception_check().unwrap_or(false) {
                    if let Err(detail_err) = env.exception_describe() {
                        warn!("Also failed to describe exception: {:?}", detail_err);
                    }
                    env.exception_clear().ok();
                }
            }
        }
    }
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
                
                // Detect initial orientation
                let width = win.width();
                let height = win.height();
                state.orientation = if width > height {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                };
                info!("Initial orientation: {:?} ({}x{})", state.orientation, width, height);
                
                // Resize backend to match window
                resize_backend(state, win);
            }
        }
        MainEvent::WindowResized { .. } | MainEvent::ConfigChanged { .. } => {
            info!("Window resized or config changed (orientation/keyboard)");
            // Re-measure grid - clone window to avoid borrow checker issues
            let window = state.native_window.clone();
            if let Some(w) = window {
                // Detect orientation change
                let width = w.width();
                let height = w.height();
                let new_orientation = if width > height {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                };
                
                // Log orientation change if it changed
                if state.orientation != new_orientation {
                    info!("Orientation changed: {:?} -> {:?} ({}x{})", 
                          state.orientation, new_orientation, width, height);
                    state.orientation = new_orientation;
                }
                
                // Update buffer geometry if needed
                unsafe {
                    let native_window_ptr = w.ptr().as_ptr() as *mut ndk_sys::ANativeWindow;
                    let result = ANativeWindow_setBuffersGeometry(
                        native_window_ptr,
                        width as i32,
                        height as i32,
                        1, // WINDOW_FORMAT_RGBA_8888
                    );
                    if result != 0 {
                        warn!("Failed to update buffer format on resize, result code: {}", result);
                    }
                }
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
    let total_rows = (height_px / state.rasterizer.font_height()) as u16;
    
    // Reserve top rows for status bar (clock, battery, etc.)
    // Status bar is typically 24-48dp, which is roughly 2-3 rows at typical font sizes
    // We'll use 2 rows as default, but calculate based on available space
    let status_bar_height_px = 48.0; // Approximate status bar height in pixels
    let status_bar_rows = ((status_bar_height_px / state.rasterizer.font_height()) as u16).max(2);
    state.top_offset_rows = status_bar_rows.min(total_rows / 4); // Don't use more than 25% of screen
    
    // Reserve bottom rows for navigation bar (gesture bar, etc.)
    // Navigation bar is typically similar height to status bar
    state.bottom_offset_rows = 2; // Use 2 rows at bottom
    
    // Available rows for content (excluding status bar and navigation bar)
    let available_rows = total_rows.saturating_sub(state.top_offset_rows).saturating_sub(state.bottom_offset_rows);
    
    if cols > 0 && available_rows > 0 {
        state.terminal.backend_mut().resize(cols, available_rows);
        info!("Resized terminal to {}x{} (window: {}x{}, top offset: {} rows, bottom offset: {} rows, orientation: {:?})", 
              cols, available_rows, width_px, height_px, state.top_offset_rows, state.bottom_offset_rows, state.orientation);
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
        
        // Create a vertical layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);
        
        // Header
        let header = Paragraph::new(vec![
            Line::from(vec![ratatui::text::Span::styled(
                " Ratadroid Terminal UI ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![ratatui::text::Span::raw("")]),
            Line::from(vec![ratatui::text::Span::raw("Welcome to your Android TUI app!")]),
        ])
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
        frame.render_widget(header, chunks[0]);
        
        // Main content area
        let content = Paragraph::new(vec![
            Line::from(vec![ratatui::text::Span::raw("")]),
            Line::from(vec![ratatui::text::Span::raw("This is a Ratatui example running natively on Android.")]),
            Line::from(vec![ratatui::text::Span::raw("")]),
            Line::from(vec![ratatui::text::Span::raw("The TUI is rendered directly to the Android Surface.")]),
            Line::from(vec![ratatui::text::Span::raw("")]),
            Line::from(vec![ratatui::text::Span::raw("Touch me to interact!")]),
        ])
        .block(Block::default().borders(Borders::ALL).title("Content"))
        .alignment(Alignment::Left);
        frame.render_widget(content, chunks[1]);
        
        // Footer
        let footer = Paragraph::new(vec![
            Line::from(vec![ratatui::text::Span::raw("")]),
            Line::from(vec![ratatui::text::Span::styled(
                " Ratatui Android Runtime ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )]),
        ])
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);
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
                
                // Calculate offsets in pixels for status bar and navigation bar
                let top_offset_px = (state.top_offset_rows as f32 * state.rasterizer.font_height()) as usize;
                let bottom_offset_px = (state.bottom_offset_rows as f32 * state.rasterizer.font_height()) as usize;
                
                // Render to full buffer, but offset the rendering position
                // The rasterizer will render starting at top_offset_px and stop before bottom_offset_px
                state.rasterizer.render_to_surface_with_offset(
                    state.terminal.backend(),
                    pixels_mut,
                    stride,         // Use stride for buffer layout
                    safe_width,     // Use safe_width for bounds checking
                    safe_height,    // Use full safe_height
                    top_offset_px,  // Offset to skip status bar
                    bottom_offset_px, // Offset to skip navigation bar
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
