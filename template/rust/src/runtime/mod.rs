//! Android runtime for Ratatui applications
//!
//! This module provides the core Android lifecycle management and event handling.

mod input;
mod keyboard;
mod lifecycle;
mod rendering;
mod window;

use android_activity::{AndroidApp, InputStatus, PollEvent};
use android_activity::input::InputEvent;
use android_logger::Config as AndroidLoggerConfig;
use log::{error, info};
use ndk::native_window::NativeWindow;
use ratatui::{Terminal, backend::Backend};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent as CrosstermKeyEvent, KeyModifiers};
use std::time::Duration;
use std::path::PathBuf;

use crate::{
    AndroidBackend, Rasterizer, DirectKeyboard, DirectKeyboardState, KeyboardState,
    warm_cache, RatadroidApp, RatadroidContext, Orientation, get_app_factory,
};

// Re-export keyboard functions
pub use keyboard::{is_soft_keyboard_visible, show_soft_keyboard, hide_soft_keyboard};

// Re-export lifecycle functions
pub use lifecycle::{save_state, restore_state};

// Re-export window functions
pub use window::WindowManager;

// Re-export input functions
pub use input::keyboard_key_to_event;

use self::keyboard::check_keyboard_visibility_changed;
use self::lifecycle::handle_lifecycle;
use self::rendering::draw_tui;

/// Default font size for mobile screens
const DEFAULT_FONT_SIZE: f32 = 48.0;

/// Height of direct keyboard in pixels (2 button rows + padding)
const DIRECT_KEYBOARD_HEIGHT_PX: u32 = 120;

/// Application state structure
pub struct AppState {
    pub terminal: Terminal<AndroidBackend>,
    pub rasterizer: Rasterizer,
    pub should_quit: bool,
    pub window_manager: WindowManager,
    pub top_offset_rows: u16,
    pub bottom_offset_rows: u16,
    pub orientation: Orientation,
    pub keyboard_state: KeyboardState,
    pub direct_keyboard: DirectKeyboard,
    pub direct_keyboard_state: DirectKeyboardState,
    pub visible_height_px: u32,
    pub total_height_px: u32,
    pub status_bar_height_px: u32,
    pub nav_bar_height_px: u32,
    pub needs_draw: bool,
    pub data_dir: PathBuf,
    // The user's app
    pub app: Box<dyn RatadroidApp>,
}

/// Android NativeActivity entry point
#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn android_main(android_app: AndroidApp) {
    // Initialize JNI utilities early
    if let Err(e) = crate::jni_utils::init_java_vm() {
        error!("Failed to initialize JavaVM: {}", e);
        // Continue anyway - some operations may still work
    }
    
    android_logger::init_once(AndroidLoggerConfig::default().with_max_level(log::LevelFilter::Info));
    
    // Get the app factory, or use demo app as fallback
    let app: Box<dyn RatadroidApp> = match get_app_factory() {
        Some(f) => f(),
        None => {
            info!("No app factory registered, using demo app");
            Box::new(crate::demo::DemoApp::new())
        }
    };
    
    info!("{} starting on Android", app.name());
    
    // Set up panic hook
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("PANIC: {:?}", panic_info);
        log::error!("PANIC: {:?}", panic_info);
        if let Some(location) = panic_info.location() {
            log::error!("Panic at {}:{}:{}", location.file(), location.line(), location.column());
        }
    }));
    
    // Initialize tokio runtime
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime: {:?}", e);
            return;
        }
    };
    
    let local = tokio::task::LocalSet::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rt.block_on(local.run_until(async_main(android_app, app)))
    }));
    
    match result {
        Ok(Ok(())) => info!("App exited normally"),
        Ok(Err(e)) => error!("Fatal error: {:?}", e),
        Err(panic) => error!("App panicked: {:?}", panic),
    }
}

async fn async_main(android_app: AndroidApp, app: Box<dyn RatadroidApp>) -> anyhow::Result<()> {
    info!("Initializing...");
    
    let font_size = get_font_size();
    info!("Using font size: {}", font_size);
    
    // Initialize rasterizer
    let rasterizer = Rasterizer::new(font_size);
    
    // Warm character cache
    warm_cache(font_size);
    
    // Get Android data directory
    let data_dir = match get_android_data_dir(&android_app) {
        Some(dir) => dir,
        None => {
            return Err(anyhow::anyhow!("Failed to get Android data directory"));
        }
    };
    info!("Data directory: {:?}", data_dir);
    
    // Initialize terminal
    let backend = AndroidBackend::new(1, 1);
    let terminal = Terminal::new(backend).map_err(|e| {
        anyhow::anyhow!("Failed to create terminal: {:?}", e)
    })?;
    
    let mut state = AppState {
        terminal,
        rasterizer,
        should_quit: false,
        window_manager: WindowManager::new(),
        top_offset_rows: 2,
        bottom_offset_rows: 2,
        orientation: Orientation::Portrait,
        keyboard_state: KeyboardState::new(),
        direct_keyboard: DirectKeyboard::new(),
        direct_keyboard_state: DirectKeyboardState::new(),
        visible_height_px: 0,
        total_height_px: 0,
        status_bar_height_px: 48,
        nav_bar_height_px: 48,
        needs_draw: true,
        data_dir: data_dir.clone(),
        app,
    };
    
    // Create context for app initialization
    let ctx = RatadroidContext {
        should_quit: false,
        needs_draw: true,
        data_dir: data_dir.clone(),
        orientation: Orientation::Portrait,
        cols: 1,
        rows: 1,
        font_width: state.rasterizer.font_width(),
        font_height: state.rasterizer.font_height(),
    };
    
    // Initialize user app
    state.app.init(&ctx)?;
    
    // Set global state pointer for JNI callbacks
    unsafe {
        lifecycle::set_global_state(&mut state as *mut AppState);
    }
    
    // Main event loop
    let mut tick_counter = 0u32;
    loop {
        tick_counter += 1;
        if tick_counter % 10 == 0 {
            tokio::task::yield_now().await;
        }
        
        // Process input events
        if let Ok(mut input_iter) = android_app.input_events_iter() {
            let window_height = state.window_manager.get_window()
                .map(|w| w.height() as f32)
                .unwrap_or(0.0);
            
            while input_iter.next(|input_event| {
                process_input_event(&mut state, &android_app, input_event, window_height)
            }) {}
        }
        
        // Check for keyboard visibility changes
        if check_keyboard_visibility_changed() {
            let new_visible_height = keyboard::get_visible_height();
            state.window_manager.update_visible_height(new_visible_height);
            state.visible_height_px = new_visible_height;
            
            let window = state.window_manager.get_window().cloned();
            if let Some(w) = window {
                resize_backend(&mut state, &w);
            }
            state.needs_draw = true;
        }
        
        // Call app tick
        {
            let mut ctx = create_context(&state);
            state.app.tick(&mut ctx);
            if ctx.should_quit {
                state.should_quit = true;
            }
            state.needs_draw = ctx.needs_draw || state.needs_draw;
        }
        
        // Poll Android events
        android_app.poll_events(Some(Duration::from_millis(16)), |event| {
            match event {
                PollEvent::Main(main_event) => {
                    handle_lifecycle(&mut state, &android_app, main_event);
                }
                PollEvent::Wake => {
                    state.needs_draw = true;
                }
                _ => {}
            }
        });
        
        if state.should_quit {
            break;
        }
        
        // Render
        if state.needs_draw {
            let window = state.window_manager.get_window().cloned();
            if let Some(w) = window {
                let backend_size = state.terminal.backend().size().unwrap_or_default();
                if backend_size.width <= 1 || backend_size.height <= 1 {
                    resize_backend(&mut state, &w);
                    state.needs_draw = true;
                } else {
                    draw_tui(&mut state, &w);
                    state.needs_draw = false;
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    
    // Clear global state pointer
    lifecycle::clear_global_state();
    
    info!("App exiting");
    Ok(())
}

fn process_input_event(
    state: &mut AppState,
    app: &AndroidApp,
    input_event: &InputEvent,
    window_height: f32,
) -> InputStatus {
    let top_offset_px = state.top_offset_rows as f32 * state.rasterizer.font_height();
    let bottom_offset_px = state.bottom_offset_rows as f32 * state.rasterizer.font_height();
    
    // Handle keyboard touch events
    let mut keyboard_handled = false;
    if let InputEvent::MotionEvent(motion) = input_event {
        use android_activity::input::MotionAction;
        if motion.action() == MotionAction::Down {
            if let Some(pointer) = motion.pointers().next() {
                let touch_x = pointer.x() as usize;
                let touch_y = pointer.y() as usize;
                
                let (window_width, window_height) = state.window_manager.get_window()
                    .map(|w| (w.width() as usize, w.height() as usize))
                    .unwrap_or((0, 0));
                
                let nav_bar_px = state.nav_bar_height_px as usize;
                let keyboard_y = window_height.saturating_sub(DIRECT_KEYBOARD_HEIGHT_PX as usize + nav_bar_px);
                let button_height = (DIRECT_KEYBOARD_HEIGHT_PX / 2).saturating_sub(4).max(20);
                
                if touch_y >= keyboard_y && window_width > 0 {
                    let key_name = state.direct_keyboard.handle_touch(
                        touch_x, touch_y, window_width, keyboard_y, button_height,
                    );
                    
                    if let Some(key_name) = key_name {
                        info!("Direct keyboard: {}", key_name);
                        state.direct_keyboard_state.set_pressed(key_name.to_string());
                        
                        if key_name == "SHIFT" {
                            state.direct_keyboard_state.toggle_shift();
                        } else if key_name == "CTRL" {
                            state.direct_keyboard_state.toggle_ctrl();
                        } else if key_name == "KEYBOARD" {
                            // Spawn keyboard show on a separate thread to avoid blocking input
                            std::thread::spawn(|| {
                                keyboard::show_soft_keyboard();
                            });
                        } else {
                            if let Some(event) = keyboard_key_to_event(key_name) {
                                let mut ctx = create_context(state);
                                state.app.handle_event(event, &mut ctx);
                                if ctx.should_quit {
                                    state.should_quit = true;
                                }
                                state.needs_draw = ctx.needs_draw || state.needs_draw;
                            }
                        }
                        
                        keyboard_handled = true;
                        state.needs_draw = true;
                    }
                }
            }
        }
    }
    
    // Handle other input events
    if !keyboard_handled {
        // Handle Back button
        if let InputEvent::KeyEvent(key) = input_event {
            use android_activity::input::{KeyAction, Keycode};
            if key.key_code() == Keycode::Back {
                // If keyboard is visible, hide it first
                if keyboard::is_soft_keyboard_visible() {
                    if key.action() == KeyAction::Down {
                        info!("Back pressed: hiding keyboard");
                        keyboard::hide_soft_keyboard();
                    }
                    // Consume Back events when keyboard is visible
                    return InputStatus::Handled;
                }
                
                // Keyboard not visible - send Esc to app
                if key.action() == KeyAction::Down {
                    info!("Back pressed: sending Esc to app");
                    let esc_event = CrosstermEvent::Key(CrosstermKeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
                    let mut ctx = create_context(state);
                    state.app.handle_event(esc_event, &mut ctx);
                    if ctx.should_quit {
                        state.should_quit = true;
                    }
                    state.needs_draw = ctx.needs_draw || state.needs_draw;
                }
                // Always consume Back events (both DOWN and UP)
                return InputStatus::Handled;
            }
        }
        
        if let Some(crossterm_event) = input::map_android_event(
            input_event,
            app,
            state.rasterizer.font_width(),
            state.rasterizer.font_height(),
            top_offset_px,
            bottom_offset_px,
            window_height,
        ) {
            let mut ctx = create_context(state);
            state.app.handle_event(crossterm_event, &mut ctx);
            if ctx.should_quit {
                state.should_quit = true;
            }
            state.needs_draw = ctx.needs_draw || state.needs_draw;
        }
    }
    
    InputStatus::Handled
}

fn create_context(state: &AppState) -> RatadroidContext {
    let backend_size = state.terminal.backend().size().unwrap_or_default();
    RatadroidContext {
        should_quit: false,
        needs_draw: false,
        data_dir: state.data_dir.clone(),
        orientation: state.orientation,
        cols: backend_size.width,
        rows: backend_size.height,
        font_width: state.rasterizer.font_width(),
        font_height: state.rasterizer.font_height(),
    }
}

pub fn resize_backend(state: &mut AppState, window: &NativeWindow) {
    let width_px = window.width() as f32;
    let height_px = if state.visible_height_px > 0 {
        state.visible_height_px as f32
    } else {
        window.height() as f32
    };
    
    let cols = (width_px / state.rasterizer.font_width()) as u16;
    let total_rows = (height_px / state.rasterizer.font_height()) as u16;
    
    let status_bar_rows = ((state.status_bar_height_px as f32 / state.rasterizer.font_height()).ceil() as u16).max(1);
    state.top_offset_rows = status_bar_rows.min(total_rows / 4);
    
    let keyboard_rows = ((DIRECT_KEYBOARD_HEIGHT_PX as f32 / state.rasterizer.font_height()).ceil() as u16).max(2);
    let nav_bar_rows = ((state.nav_bar_height_px as f32 / state.rasterizer.font_height()).ceil() as u16).max(1);
    state.bottom_offset_rows = keyboard_rows + nav_bar_rows;
    
    let available_rows = total_rows.saturating_sub(state.top_offset_rows).saturating_sub(state.bottom_offset_rows);
    
    if cols > 0 && available_rows > 0 {
        state.terminal.backend_mut().resize(cols, available_rows);
        info!("Resized to {}x{} (visible: {}x{})", cols, available_rows, width_px, height_px);
        
        // Notify app of resize
        let ctx = create_context(state);
        state.app.on_resize(cols, available_rows, &ctx);
        
        let _ = state.terminal.clear();
    }
}

pub fn handle_window_resize(state: &mut AppState) {
    let window = state.window_manager.get_window().cloned();
    if let Some(w) = window {
        let width = w.width();
        let height = w.height();
        
        state.window_manager.handle_resize(width, height);
        
        // Update state from window manager
        state.orientation = state.window_manager.orientation;
        state.total_height_px = state.window_manager.height_px;
        
        if state.visible_height_px == 0 {
            state.visible_height_px = state.window_manager.height_px;
        }
        
        resize_backend(state, &w);
        state.needs_draw = true;
    }
}

/// Get font size from environment or use default
pub fn get_font_size() -> f32 {
    std::env::var("RATADROID_FONT_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_FONT_SIZE)
}

/// Get the Android app's data directory using JNI
pub fn get_android_data_dir(_app: &AndroidApp) -> Option<PathBuf> {
    use jni::objects::{JObject, JString};
    use ndk_context::android_context;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let activity_obj = ctx.context();
    
    if vm_ptr.is_null() || activity_obj.is_null() {
        return None;
    }
    
    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr as *mut _).ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let context_jobj = JObject::from_raw(activity_obj as jni::sys::jobject);
        
        let files_dir_obj = env.call_method(&context_jobj, "getFilesDir", "()Ljava/io/File;", &[])
            .ok()?.l().ok()?;
        
        let path_jstring = env.call_method(&files_dir_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .ok()?.l().ok()?;
        
        let path_jstring_ref: JString = path_jstring.into();
        let java_str = env.get_string(&path_jstring_ref).ok()?;
        
        Some(PathBuf::from(java_str.to_string_lossy().to_string()))
    }
}

