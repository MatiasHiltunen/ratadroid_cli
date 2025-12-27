//! Android runtime for Ratatui applications
//!
//! This module provides the core Android lifecycle management and event handling.

use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use android_activity::input::InputEvent;
use android_logger::Config as AndroidLoggerConfig;
use log::{error, info, warn};
use ndk::native_window::NativeWindow;
use ndk_sys::ANativeWindow_setBuffersGeometry;
use jni::objects::JObject;
use jni::sys::jobject;
use ratatui::{Terminal, backend::Backend};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent as CrosstermKeyEvent, KeyModifiers};
use std::time::Duration;
use std::path::PathBuf;

use crate::{
    AndroidBackend, Rasterizer, DirectKeyboard, DirectKeyboardState, KeyboardState,
    warm_cache, RatadroidApp, RatadroidContext, Orientation, get_app_factory,
};

/// Default font size for mobile screens
const DEFAULT_FONT_SIZE: f32 = 48.0;

/// Height of direct keyboard in pixels (2 button rows + padding)
const DIRECT_KEYBOARD_HEIGHT_PX: u32 = 80;

/// Global atomic flag to signal keyboard visibility changed from Java
static KEYBOARD_VISIBILITY_CHANGED: std::sync::atomic::AtomicBool = 
    std::sync::atomic::AtomicBool::new(false);

/// Global atomic for visible height when keyboard visibility changes
static KEYBOARD_VISIBLE_HEIGHT: std::sync::atomic::AtomicU32 = 
    std::sync::atomic::AtomicU32::new(0);

/// JNI callback from Java when keyboard visibility changes
/// Java signature: private native void notifyKeyboardVisibilityChanged(boolean visible, int visibleHeight);
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_ratadroid_ratadroid_1template_NativeActivity_notifyKeyboardVisibilityChanged(
    _env: jni::JNIEnv,
    _class: jni::objects::JObject,
    visible: jni::sys::jboolean,
    visible_height: jni::sys::jint,
) {
    let is_visible = visible != 0;
    log::info!("JNI: Keyboard visibility changed - visible={}, height={}px", is_visible, visible_height);
    KEYBOARD_VISIBLE_HEIGHT.store(visible_height as u32, std::sync::atomic::Ordering::SeqCst);
    KEYBOARD_VISIBILITY_CHANGED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Get font size from environment or use default
pub fn get_font_size() -> f32 {
    std::env::var("RATADROID_FONT_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_FONT_SIZE)
}

/// Application state structure
struct AppState {
    terminal: Terminal<AndroidBackend>,
    rasterizer: Rasterizer,
    should_quit: bool,
    native_window: Option<NativeWindow>,
    top_offset_rows: u16,
    bottom_offset_rows: u16,
    orientation: Orientation,
    keyboard_state: KeyboardState,
    direct_keyboard: DirectKeyboard,
    direct_keyboard_state: DirectKeyboardState,
    visible_height_px: u32,
    total_height_px: u32,
    status_bar_height_px: u32,
    nav_bar_height_px: u32,
    needs_draw: bool,
    data_dir: PathBuf,
    // The user's app
    app: Box<dyn RatadroidApp>,
}

/// Android NativeActivity entry point
#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn android_main(android_app: AndroidApp) {
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

async fn async_main(android_app: AndroidApp, mut app: Box<dyn RatadroidApp>) -> anyhow::Result<()> {
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
    
    // Get screen insets
    let (status_bar_height, nav_bar_height) = get_screen_insets();
    info!("Screen insets: status_bar={}px, nav_bar={}px", status_bar_height, nav_bar_height);
    
    // Initialize terminal
    let backend = AndroidBackend::new(1, 1);
    let terminal = Terminal::new(backend).map_err(|e| {
        anyhow::anyhow!("Failed to create terminal: {:?}", e)
    })?;
    
    let mut state = AppState {
        terminal,
        rasterizer,
        should_quit: false,
        native_window: None,
        top_offset_rows: 2,
        bottom_offset_rows: 2,
        orientation: Orientation::Portrait,
        keyboard_state: KeyboardState::new(),
        direct_keyboard: DirectKeyboard::new(),
        direct_keyboard_state: DirectKeyboardState::new(),
        visible_height_px: 0,
        total_height_px: 0,
        status_bar_height_px: status_bar_height,
        nav_bar_height_px: nav_bar_height,
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
    
    // Main event loop
    let mut tick_counter = 0u32;
    loop {
        tick_counter += 1;
        if tick_counter % 10 == 0 {
            tokio::task::yield_now().await;
        }
        
        // Process input events
        if let Ok(mut input_iter) = android_app.input_events_iter() {
            let window_height = state.native_window.as_ref()
                .map(|w| w.height() as f32)
                .unwrap_or(0.0);
            
            while input_iter.next(|input_event| {
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
                            
                            let (window_width, window_height) = state.native_window.as_ref()
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
                                        show_soft_keyboard(&android_app);
                                    } else {
                                        if let Some(event) = keyboard_key_to_event(key_name) {
                                            let mut ctx = create_context(&state);
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
                    if let Some(crossterm_event) = map_android_event(
                        input_event,
                        &android_app,
                        state.rasterizer.font_width(),
                        state.rasterizer.font_height(),
                        top_offset_px,
                        bottom_offset_px,
                        window_height,
                    ) {
                        let mut ctx = create_context(&state);
                        state.app.handle_event(crossterm_event, &mut ctx);
                        if ctx.should_quit {
                            state.should_quit = true;
                        }
                        state.needs_draw = ctx.needs_draw || state.needs_draw;
                    }
                }
                
                InputStatus::Handled
            }) {}
        }
        
        // Check for keyboard visibility changes
        if KEYBOARD_VISIBILITY_CHANGED.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let new_visible_height = KEYBOARD_VISIBLE_HEIGHT.load(std::sync::atomic::Ordering::SeqCst);
            let old_visible = state.visible_height_px;
            state.visible_height_px = new_visible_height;
            
            if old_visible != new_visible_height {
                info!("Visible height: {} -> {}", old_visible, new_visible_height);
                let window = state.native_window.clone();
                if let Some(ref w) = window {
                    resize_backend(&mut state, w);
                }
                state.needs_draw = true;
            }
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
            let window = state.native_window.clone();
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
    
    info!("App exiting");
    Ok(())
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

fn keyboard_key_to_event(key_name: &str) -> Option<CrosstermEvent> {
    let key_code = match key_name {
        "ESC" => KeyCode::Esc,
        "TAB" => KeyCode::Tab,
        "SHIFT" | "CTRL" | "KEYBOARD" => return None,
        "UP" => KeyCode::Up,
        "DOWN" => KeyCode::Down,
        "LEFT" => KeyCode::Left,
        "RIGHT" => KeyCode::Right,
        "ENTER" => KeyCode::Enter,
        "SPACE" => KeyCode::Char(' '),
        "BACKSPACE" => KeyCode::Backspace,
        "DELETE" => KeyCode::Delete,
        _ => return None,
    };
    Some(CrosstermEvent::Key(CrosstermKeyEvent::new(key_code, KeyModifiers::empty())))
}

/// Map Android input events to crossterm events
fn map_android_event(
    event: &InputEvent,
    app: &AndroidApp,
    font_width: f32,
    font_height: f32,
    top_offset_px: f32,
    bottom_offset_px: f32,
    window_height: f32,
) -> Option<CrosstermEvent> {
    use android_activity::input::{KeyAction, MotionAction};
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    
    match event {
        InputEvent::KeyEvent(key) => {
            if key.action() == KeyAction::Down {
                map_keycode(key.key_code(), key.meta_state(), app)
                    .map(CrosstermEvent::Key)
            } else {
                None
            }
        }
        InputEvent::MotionEvent(motion) => {
            let action = motion.action();
            let pointer = motion.pointers().next()?;
            
            let adjusted_y = (pointer.y() - top_offset_px).max(0.0);
            let max_content_y = window_height - bottom_offset_px;
            if pointer.y() >= max_content_y {
                return None;
            }
            
            let col = (pointer.x() / font_width) as u16;
            let row = (adjusted_y / font_height) as u16;
            
            let kind = match action {
                MotionAction::Down => MouseEventKind::Down(MouseButton::Left),
                MotionAction::Up => MouseEventKind::Up(MouseButton::Left),
                MotionAction::Move => MouseEventKind::Drag(MouseButton::Left),
                _ => return None,
            };
            
            Some(CrosstermEvent::Mouse(MouseEvent {
                kind,
                column: col,
                row,
                modifiers: KeyModifiers::empty(),
            }))
        }
        _ => None,
    }
}

fn map_keycode(
    key_code: android_activity::input::Keycode,
    _meta: android_activity::input::MetaState,
    _app: &AndroidApp,
) -> Option<CrosstermKeyEvent> {
    use android_activity::input::Keycode;
    
    let key = match key_code {
        Keycode::A => KeyCode::Char('a'),
        Keycode::B => KeyCode::Char('b'),
        Keycode::C => KeyCode::Char('c'),
        Keycode::D => KeyCode::Char('d'),
        Keycode::E => KeyCode::Char('e'),
        Keycode::F => KeyCode::Char('f'),
        Keycode::G => KeyCode::Char('g'),
        Keycode::H => KeyCode::Char('h'),
        Keycode::I => KeyCode::Char('i'),
        Keycode::J => KeyCode::Char('j'),
        Keycode::K => KeyCode::Char('k'),
        Keycode::L => KeyCode::Char('l'),
        Keycode::M => KeyCode::Char('m'),
        Keycode::N => KeyCode::Char('n'),
        Keycode::O => KeyCode::Char('o'),
        Keycode::P => KeyCode::Char('p'),
        Keycode::Q => KeyCode::Char('q'),
        Keycode::R => KeyCode::Char('r'),
        Keycode::S => KeyCode::Char('s'),
        Keycode::T => KeyCode::Char('t'),
        Keycode::U => KeyCode::Char('u'),
        Keycode::V => KeyCode::Char('v'),
        Keycode::W => KeyCode::Char('w'),
        Keycode::X => KeyCode::Char('x'),
        Keycode::Y => KeyCode::Char('y'),
        Keycode::Z => KeyCode::Char('z'),
        Keycode::Keycode0 => KeyCode::Char('0'),
        Keycode::Keycode1 => KeyCode::Char('1'),
        Keycode::Keycode2 => KeyCode::Char('2'),
        Keycode::Keycode3 => KeyCode::Char('3'),
        Keycode::Keycode4 => KeyCode::Char('4'),
        Keycode::Keycode5 => KeyCode::Char('5'),
        Keycode::Keycode6 => KeyCode::Char('6'),
        Keycode::Keycode7 => KeyCode::Char('7'),
        Keycode::Keycode8 => KeyCode::Char('8'),
        Keycode::Keycode9 => KeyCode::Char('9'),
        Keycode::Space => KeyCode::Char(' '),
        Keycode::Enter => KeyCode::Enter,
        Keycode::Escape => KeyCode::Esc,
        Keycode::Tab => KeyCode::Tab,
        Keycode::Del => KeyCode::Backspace,
        Keycode::ForwardDel => KeyCode::Delete,
        Keycode::DpadUp => KeyCode::Up,
        Keycode::DpadDown => KeyCode::Down,
        Keycode::DpadLeft => KeyCode::Left,
        Keycode::DpadRight => KeyCode::Right,
        Keycode::Home => KeyCode::Home,
        Keycode::MoveEnd => KeyCode::End,
        Keycode::PageUp => KeyCode::PageUp,
        Keycode::PageDown => KeyCode::PageDown,
        _ => return None,
    };
    
    Some(CrosstermKeyEvent::new(key, KeyModifiers::empty()))
}

fn resize_backend(state: &mut AppState, window: &NativeWindow) {
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

fn draw_tui(state: &mut AppState, window: &NativeWindow) {
    let _ = state.terminal.clear();
    
    // Draw app UI
    let ctx = create_context(state);
    let _ = state.terminal.draw(|frame| {
        state.app.draw(frame, &ctx);
    });
    
    // Blit to screen
    match window.lock(None) {
        Ok(mut buffer) => {
            let stride = buffer.stride() as usize;
            let height = buffer.height() as usize;
            let bits_ptr = buffer.bits();
            
            if !bits_ptr.is_null() {
                let window_width = window.width() as usize;
                let window_height = window.height() as usize;
                let safe_height = height.min(window_height);
                
                if safe_height == 0 {
                    return;
                }
                
                let max_buffer_size = stride.saturating_mul(safe_height).saturating_mul(4);
                if max_buffer_size == 0 {
                    return;
                }
                
                let pixels_mut = unsafe {
                    std::slice::from_raw_parts_mut(bits_ptr as *mut u8, max_buffer_size)
                };
                
                // Clear to black
                for chunk in pixels_mut.chunks_exact_mut(4) {
                    chunk[0] = 0; chunk[1] = 0; chunk[2] = 0; chunk[3] = 255;
                }
                
                let top_offset_px = (state.top_offset_rows as f32 * state.rasterizer.font_height()) as usize;
                let bottom_offset_px = (state.bottom_offset_rows as f32 * state.rasterizer.font_height()) as usize;
                
                state.rasterizer.render_to_surface_with_offset(
                    state.terminal.backend(),
                    pixels_mut,
                    stride,
                    window_width,
                    window_height,
                    top_offset_px,
                    bottom_offset_px,
                );
                
                // Render direct keyboard
                let nav_bar_px = state.nav_bar_height_px as usize;
                let keyboard_y = window_height.saturating_sub(DIRECT_KEYBOARD_HEIGHT_PX as usize + nav_bar_px);
                let button_height = (DIRECT_KEYBOARD_HEIGHT_PX / 2).saturating_sub(4).max(20);
                
                state.direct_keyboard.render(
                    &state.direct_keyboard_state,
                    pixels_mut,
                    stride,
                    window_width,
                    window_height,
                    keyboard_y,
                    button_height,
                );
            }
        }
        Err(e) => {
            warn!("Failed to lock buffer: {:?}", e);
        }
    }
}

fn handle_lifecycle(state: &mut AppState, app: &AndroidApp, event: MainEvent) {
    match event {
        MainEvent::InitWindow { .. } => {
            info!("Window initialized");
            if let Some(win) = app.native_window().as_ref() {
                let width = win.width();
                let height = win.height();
                
                state.total_height_px = height as u32;
                state.visible_height_px = height as u32;
                
                let (status_bar, nav_bar) = get_screen_insets();
                state.status_bar_height_px = status_bar;
                state.nav_bar_height_px = nav_bar;
                
                unsafe {
                    let native_window_ptr = win.ptr().as_ptr() as *mut ndk_sys::ANativeWindow;
                    ANativeWindow_setBuffersGeometry(native_window_ptr, width as i32, height as i32, 1);
                }
                
                state.native_window = Some(win.clone());
                state.orientation = if width > height { Orientation::Landscape } else { Orientation::Portrait };
                
                resize_backend(state, win);
                state.needs_draw = true;
            }
        }
        MainEvent::ContentRectChanged { .. } => {
            handle_window_resize(state);
        }
        MainEvent::WindowResized { .. } | MainEvent::ConfigChanged { .. } => {
            handle_window_resize(state);
        }
        MainEvent::Destroy => {
            state.should_quit = true;
        }
        _ => {}
    }
}

fn handle_window_resize(state: &mut AppState) {
    let window = state.native_window.clone();
    if let Some(w) = window {
        let width = w.width();
        let height = w.height();
        
        let new_orientation = if width > height { Orientation::Landscape } else { Orientation::Portrait };
        
        if state.orientation != new_orientation {
            state.orientation = new_orientation;
            state.total_height_px = height as u32;
            state.visible_height_px = height as u32;
        }
        
        if state.visible_height_px == 0 {
            state.visible_height_px = height as u32;
        }
        
        unsafe {
            let native_window_ptr = w.ptr().as_ptr() as *mut ndk_sys::ANativeWindow;
            ANativeWindow_setBuffersGeometry(native_window_ptr, width as i32, height as i32, 1);
        }
        
        resize_backend(state, &w);
        state.needs_draw = true;
    }
}

/// Get the Android app's data directory using JNI
pub fn get_android_data_dir(_app: &AndroidApp) -> Option<PathBuf> {
    use ndk_context::android_context;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let context_obj = ctx.context();
    
    if vm_ptr.is_null() || context_obj.is_null() {
        return None;
    }
    
    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr as *mut _).ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let context_jobj = JObject::from_raw(context_obj as jobject);
        
        let files_dir_obj = env.call_method(context_jobj, "getFilesDir", "()Ljava/io/File;", &[])
            .ok()?.l().ok()?;
        
        let path_jstring = env.call_method(files_dir_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .ok()?.l().ok()?;
        
        let path_jstring_ref: jni::objects::JString = path_jstring.into();
        let java_str = env.get_string(&path_jstring_ref).ok()?;
        
        Some(PathBuf::from(java_str.to_string_lossy().to_string()))
    }
}

/// Show the Android soft keyboard
pub fn show_soft_keyboard(_app: &AndroidApp) {
    use ndk_context::android_context;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let context_obj = ctx.context();
    
    if vm_ptr.is_null() || context_obj.is_null() {
        return;
    }
    
    unsafe {
        let vm = match jni::JavaVM::from_raw(vm_ptr as *mut _) {
            Ok(vm) => vm,
            Err(_) => return,
        };
        
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(_) => return,
        };
        
        let context_jobj = JObject::from_raw(context_obj as jobject);
        
        let input_method_service = match env.new_string("input_method") {
            Ok(s) => s,
            Err(_) => return,
        };
        
        let imm_obj = match env.call_method(
            context_jobj,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[(&input_method_service).into()]
        ) {
            Ok(obj) => match obj.l() {
                Ok(o) => o,
                Err(_) => return,
            },
            Err(_) => return,
        };
        
        let _ = env.call_method(imm_obj, "toggleSoftInput", "(II)V", &[2i32.into(), 0i32.into()]);
    }
}

/// Get screen insets (status bar, navigation bar)
pub fn get_screen_insets() -> (u32, u32) {
    use ndk_context::android_context;
    use jni::objects::JPrimitiveArray;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let context_obj = ctx.context();
    
    if vm_ptr.is_null() || context_obj.is_null() {
        return (48, 48);
    }
    
    unsafe {
        let vm = match jni::JavaVM::from_raw(vm_ptr as *mut _) {
            Ok(vm) => vm,
            Err(_) => return (48, 48),
        };
        
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(_) => return (48, 48),
        };
        
        let context_jobj = JObject::from_raw(context_obj as jobject);
        
        match env.call_method(context_jobj, "getScreenInsets", "()[I", &[]) {
            Ok(result) => {
                if let Ok(array_obj) = result.l() {
                    let int_array: JPrimitiveArray<i32> = JPrimitiveArray::from_raw(array_obj.into_raw());
                    let len = env.get_array_length(&int_array).unwrap_or(0) as usize;
                    
                    if len >= 2 {
                        let mut insets = vec![0i32; len];
                        if env.get_int_array_region(&int_array, 0, &mut insets).is_ok() {
                            return (insets[0].max(0) as u32, insets[1].max(0) as u32);
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }
    
    (48, 48)
}

