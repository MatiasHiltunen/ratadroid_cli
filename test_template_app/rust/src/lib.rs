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
use jni::objects::JObject;
#[cfg(target_os = "android")]
use jni::sys::jobject;
#[cfg(target_os = "android")]
use ndk_context;
#[cfg(target_os = "android")]
use ndk::native_window::NativeWindow;
#[cfg(target_os = "android")]
use ndk_sys::ANativeWindow_setBuffersGeometry;
#[cfg(target_os = "android")]
use ratatui::Terminal;
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
    todo_app: todo_app::TodoApp,
    top_offset_rows: u16, // Number of rows to skip at top for status bar
    orientation: Orientation, // Current screen orientation
}

// Note: We no longer need to store ANativeActivity pointers
// ndk-context provides access to the activity context when needed

/// Android NativeActivity entry point
/// android-activity crate with "native-activity" feature bridges ANativeActivity_onCreate
/// to this function automatically
#[cfg(target_os = "android")]
#[no_mangle]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn android_main(app: AndroidApp) {
    android_logger::init_once(Config::default().with_max_level(log::LevelFilter::Info));
    info!("Ratatui Android Runtime starting");
    
    // Try to initialize ndk-context early - it needs to be initialized before use
    // This allows us to access the activity context for JNI calls
    #[cfg(target_os = "android")]
    {
        let _ctx_result = std::panic::catch_unwind(|| {
            let _ctx = ndk_context::android_context();
            info!("ndk-context initialized successfully");
        });
        if _ctx_result.is_err() {
            warn!("ndk-context initialization failed - keyboard support may be limited");
        }
    }

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
    let mut todo_app = todo_app::TodoApp::new();
    #[cfg(target_os = "android")]
    todo_app.set_android_app(&app);
    
    let mut state = AppState {
        terminal: Terminal::new(backend).unwrap(),
        rasterizer,
        should_quit: false,
        native_window: None,
        todo_app,
        top_offset_rows: 2, // Default: skip 2 rows at top for status bar
        orientation: Orientation::Portrait, // Default orientation
    };

    // Main event loop
    loop {
        // Poll input events first - this prevents ANR (Application Not Responding)
        if let Ok(mut input_iter) = app.input_events_iter() {
            while input_iter.next(|input_event| {
                // Calculate top offset in pixels for input coordinate adjustment
                let top_offset_px = state.top_offset_rows as f32 * state.rasterizer.font_height();
                
                // Map Android Input -> TUI Event
                match input::map_android_event(
                    input_event,
                    &app,
                    &mut input_state,
                    state.rasterizer.font_width(),
                    state.rasterizer.font_height(),
                    top_offset_px,
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

/// Show the Android soft keyboard by calling Java method via JNI
/// Uses ndk-context to get activity object (works with android-activity)
/// Wrapped in panic handler to prevent crashes when called from input event handler
#[cfg(target_os = "android")]
fn show_soft_keyboard(_app: &AndroidApp) {
    info!("show_soft_keyboard() called - attempting to show keyboard");
    
    // Wrap in panic handler to prevent crashes if ndk-context isn't initialized
    // This can happen when called from input event handler threads
    let result = std::panic::catch_unwind(|| {
        // Use ndk-context to get the activity context
        // This works with android-activity without requiring ndk-glue
        // Note: android_context() returns AndroidContext directly, not Option
        let ctx = match std::panic::catch_unwind(|| ndk_context::android_context()) {
            Ok(ctx) => ctx,
            Err(_) => {
                warn!("ndk-context::android_context() panicked - trying ndk-glue fallback");
                // Fallback: try ndk-glue if available
                return try_show_keyboard_with_ndk_glue();
            }
        };
        
        info!("Got Android context from ndk-context");
        
        // Get Java VM and activity object from context
        // Note: ndk-context 0.1 API uses vm() and context() methods
        let vm_ptr = ctx.vm();
        let activity_obj = ctx.context();
        
        if vm_ptr.is_null() {
            warn!("Java VM is null from ndk-context");
            return;
        }
        
        if activity_obj.is_null() {
            warn!("Activity object is null from ndk-context");
            return;
        }
        
        info!("Got Java VM and activity object from ndk-context");
        
        unsafe {
            // Attach to Java VM
            let vm = match jni::JavaVM::from_raw(vm_ptr as *mut _) {
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
    });
    
    match result {
        Ok(()) => {
            info!("show_soft_keyboard completed successfully");
        }
        Err(_) => {
            warn!("show_soft_keyboard panicked - trying ndk-glue fallback");
            try_show_keyboard_with_ndk_glue();
        }
    }
}

/// Fallback method to show keyboard using ndk-glue if ndk-context fails
#[cfg(target_os = "android")]
fn try_show_keyboard_with_ndk_glue() {
    let result = std::panic::catch_unwind(|| {
        let native_activity = ndk_glue::native_activity();
        let vm = native_activity.vm();
        let activity_obj = native_activity.activity();
        
        if vm.is_null() || activity_obj.is_null() {
            warn!("ndk-glue: Java VM or activity object is null");
            return;
        }
        
        info!("Got Java VM and activity from ndk-glue");
        
        unsafe {
            let vm = match jni::JavaVM::from_raw(vm as *mut _) {
                Ok(vm) => vm,
                Err(e) => {
                    warn!("Failed to create JavaVM from ndk-glue: {:?}", e);
                    return;
                }
            };
            
            let mut env = match vm.attach_current_thread_permanently() {
                Ok(env) => env,
                Err(e) => {
                    warn!("Failed to attach to Java VM from ndk-glue: {:?}", e);
                    return;
                }
            };
            
            let activity_jobj = JObject::from_raw(activity_obj as jobject);
            
            match env.call_method(activity_jobj, "showSoftKeyboard", "()V", &[]) {
                Ok(_) => {
                    info!("Soft keyboard shown successfully via ndk-glue fallback");
                }
                Err(e) => {
                    warn!("Failed to call showSoftKeyboard via ndk-glue: {:?}", e);
                    if env.exception_check().unwrap_or(false) {
                        env.exception_clear().ok();
                    }
                }
            }
        }
    });
    
    if result.is_err() {
        warn!("ndk-glue fallback also failed - keyboard cannot be shown");
    }
}

/// Hide the Android soft keyboard by calling Java method via JNI
/// Uses ndk-context to get activity object (works with android-activity)
/// Wrapped in panic handler to prevent crashes when called from input event handler
#[cfg(target_os = "android")]
fn hide_soft_keyboard(_app: &AndroidApp) {
    info!("hide_soft_keyboard() called - attempting to hide keyboard");
    
    // Wrap in panic handler to prevent crashes if ndk-context isn't initialized
    let _result = std::panic::catch_unwind(|| {
        // Use ndk-context to get the activity context
        // Note: android_context() returns AndroidContext directly, not Option
        let ctx = ndk_context::android_context();
        
        let vm_ptr = ctx.vm();
        let activity_obj = ctx.context();
        
        if vm_ptr.is_null() || activity_obj.is_null() {
            warn!("Java VM or activity object is null from ndk-context");
            return;
        }
        
        unsafe {
            let vm = match jni::JavaVM::from_raw(vm_ptr as *mut _) {
                Ok(vm) => vm,
                Err(e) => {
                    warn!("Failed to create JavaVM: {:?}", e);
                    return;
                }
            };
            
            let _env = match vm.attach_current_thread_permanently() {
                Ok(env) => env,
                Err(e) => {
                    warn!("Failed to attach to Java VM: {:?}", e);
                    return;
                }
            };
            
            let _activity_jobj = JObject::from_raw(activity_obj as jobject);
            
            // For now, just log that we're trying to hide the keyboard
            // A full implementation would use InputMethodManager to hide it
            info!("Attempted to hide soft keyboard");
        }
    });
    
    // Ignore panic result - non-fatal
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
    
    // Available rows for content (excluding status bar)
    let available_rows = total_rows.saturating_sub(state.top_offset_rows);
    
    if cols > 0 && available_rows > 0 {
        state.terminal.backend_mut().resize(cols, available_rows);
        info!("Resized terminal to {}x{} (window: {}x{}, top offset: {} rows, orientation: {:?})", 
              cols, available_rows, width_px, height_px, state.top_offset_rows, state.orientation);
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
        // Pass orientation info if needed for layout adjustments
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
                
                // Calculate top offset in pixels for status bar
                let top_offset_px = (state.top_offset_rows as f32 * state.rasterizer.font_height()) as usize;
                
                // Render to full buffer, but offset the rendering position
                // The rasterizer will render starting at top_offset_px
                state.rasterizer.render_to_surface_with_offset(
                    state.terminal.backend(),
                    pixels_mut,
                    stride,         // Use stride for buffer layout
                    safe_width,     // Use safe_width for bounds checking
                    safe_height,    // Use full safe_height
                    top_offset_px,  // Offset to skip status bar
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
