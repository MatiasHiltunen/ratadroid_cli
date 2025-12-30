//! Lifecycle event handling for Android NativeActivity

use android_activity::{AndroidApp, MainEvent};
use log::{info, warn};

use super::{AppState, resize_backend, handle_window_resize};
use super::keyboard;

/// Application state that can be saved/restored
#[derive(Debug, Clone)]
pub struct SavedState {
    /// Window width
    pub width_px: u32,
    /// Window height
    pub height_px: u32,
    /// Orientation
    pub orientation: crate::Orientation,
    /// Status bar height
    pub status_bar_height_px: u32,
    /// Navigation bar height
    pub nav_bar_height_px: u32,
}

impl SavedState {
    /// Serialize state to bytes
    pub fn serialize(&self) -> Vec<u8> {
        use std::io::Write;
        let mut buf = Vec::new();
        // Simple binary format: each u32 in little-endian
        buf.write_all(&self.width_px.to_le_bytes()).ok();
        buf.write_all(&self.height_px.to_le_bytes()).ok();
        buf.write_all(&(self.orientation as u8).to_le_bytes()).ok();
        buf.write_all(&self.status_bar_height_px.to_le_bytes()).ok();
        buf.write_all(&self.nav_bar_height_px.to_le_bytes()).ok();
        buf
    }

    /// Deserialize state from bytes
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 17 {
            // Need at least 4+4+1+4+4 = 17 bytes
            return None;
        }
        
        use std::io::Read;
        let mut reader = std::io::Cursor::new(data);
        let mut buf = [0u8; 4];
        
        let _ = reader.read_exact(&mut buf).ok()?;
        let width_px = u32::from_le_bytes(buf);
        
        let _ = reader.read_exact(&mut buf).ok()?;
        let height_px = u32::from_le_bytes(buf);
        
        let mut orient_buf = [0u8; 1];
        let _ = reader.read_exact(&mut orient_buf).ok()?;
        let orientation = match orient_buf[0] {
            0 => crate::Orientation::Portrait,
            1 => crate::Orientation::Landscape,
            _ => return None,
        };
        
        let _ = reader.read_exact(&mut buf).ok()?;
        let status_bar_height_px = u32::from_le_bytes(buf);
        
        let _ = reader.read_exact(&mut buf).ok()?;
        let nav_bar_height_px = u32::from_le_bytes(buf);
        
        Some(SavedState {
            width_px,
            height_px,
            orientation,
            status_bar_height_px,
            nav_bar_height_px,
        })
    }
}

/// Handle lifecycle events from Android
pub fn handle_lifecycle(
    state: &mut AppState,
    app: &AndroidApp,
    event: MainEvent,
) {
    match event {
        MainEvent::InitWindow { .. } => {
            info!("Lifecycle: InitWindow");
            if let Some(win) = app.native_window().as_ref() {
                if let Err(e) = state.window_manager.init_window(win) {
                    warn!("Failed to initialize window: {}", e);
                } else {
                    // Update state from window manager
                    state.visible_height_px = state.window_manager.visible_height_px;
                    state.orientation = state.window_manager.orientation;
                    state.status_bar_height_px = state.window_manager.status_bar_height_px;
                    state.nav_bar_height_px = state.window_manager.nav_bar_height_px;
                    
                    // Resize backend
                    let window = state.window_manager.get_window().cloned();
                    if let Some(w) = window {
                        resize_backend(state, &w);
                    }
                    state.needs_draw = true;
                }
            }
        }
        MainEvent::SaveState { .. } => {
            info!("Lifecycle: SaveState");
            // Save state is handled by the save_state callback
        }
        MainEvent::Resume { .. } => {
            info!("Lifecycle: Resume");
            state.needs_draw = true;
            // Resume rendering if paused
        }
        MainEvent::Pause { .. } => {
            info!("Lifecycle: Pause");
            // Pause rendering if needed (optional optimization)
        }
        MainEvent::LostFocus => {
            info!("Lifecycle: LostFocus");
            // Could pause rendering here for performance
        }
        MainEvent::GainedFocus => {
            info!("Lifecycle: GainedFocus");
            state.needs_draw = true;
            // Resume rendering
        }
        MainEvent::ContentRectChanged { .. } => {
            info!("Lifecycle: ContentRectChanged");
            // Detect keyboard visibility based on window size change
            if let Some(w) = state.window_manager.get_window() {
                let current_height = w.height() as u32;
                let screen_height = state.window_manager.height_px.max(current_height);
                keyboard::detect_keyboard_visibility(current_height, screen_height);
            }
            handle_window_resize(state);
        }
        MainEvent::WindowResized { .. } | MainEvent::ConfigChanged { .. } => {
            info!("Lifecycle: WindowResized/ConfigChanged");
            // Detect keyboard visibility based on window size change
            if let Some(w) = state.window_manager.get_window() {
                let current_height = w.height() as u32;
                let screen_height = state.window_manager.height_px.max(current_height);
                keyboard::detect_keyboard_visibility(current_height, screen_height);
            }
            handle_window_resize(state);
        }
        MainEvent::Destroy => {
            info!("Lifecycle: Destroy");
            state.should_quit = true;
            // Cleanup will happen when state is dropped
        }
        _ => {
            // Other events are handled elsewhere or ignored
        }
    }
}

/// Save application state
pub fn save_state(state: &AppState) -> Option<Vec<u8>> {
    let saved = SavedState {
        width_px: state.window_manager.width_px,
        height_px: state.window_manager.height_px,
        orientation: state.window_manager.orientation,
        status_bar_height_px: state.window_manager.status_bar_height_px,
        nav_bar_height_px: state.window_manager.nav_bar_height_px,
    };
    
    Some(saved.serialize())
}

/// Restore application state
pub fn restore_state(state: &mut AppState, data: &[u8]) {
    if let Some(saved) = SavedState::deserialize(data) {
        info!("Restoring state: {}x{}px, {:?}", saved.width_px, saved.height_px, saved.orientation);
        state.window_manager.width_px = saved.width_px;
        state.window_manager.height_px = saved.height_px;
        state.window_manager.visible_height_px = saved.height_px;
        state.window_manager.orientation = saved.orientation;
        state.window_manager.status_bar_height_px = saved.status_bar_height_px;
        state.window_manager.nav_bar_height_px = saved.nav_bar_height_px;
        
        // Update state fields
        state.orientation = saved.orientation;
        state.status_bar_height_px = saved.status_bar_height_px;
        state.nav_bar_height_px = saved.nav_bar_height_px;
        state.visible_height_px = saved.height_px;
    } else {
        warn!("Failed to deserialize saved state");
    }
}

/// Wrapper for raw pointer to make it Send/Sync
/// SAFETY: We only access this from the main thread via JNI callbacks
struct AppStatePtr(*mut AppState);
unsafe impl Send for AppStatePtr {}
unsafe impl Sync for AppStatePtr {}

/// Global state for JNI callbacks (set during initialization)
static GLOBAL_STATE: std::sync::Mutex<Option<AppStatePtr>> = std::sync::Mutex::new(None);

/// Set global state pointer for JNI callbacks
pub unsafe fn set_global_state(state: *mut AppState) {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        *guard = Some(AppStatePtr(state));
    }
}

/// Clear global state pointer
pub fn clear_global_state() {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        *guard = None;
    }
}

/// JNI callback: Save native state
/// Java signature: private native byte[] saveNativeState();
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_ratadroid_template_NativeActivity_saveNativeState(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JObject<'_>,
) -> jni::sys::jbyteArray {
    
    if let Ok(guard) = GLOBAL_STATE.lock() {
        if let Some(AppStatePtr(state_ptr)) = guard.as_ref() {
            unsafe {
                let state = &**state_ptr;
                if let Some(data) = save_state(state) {
                    // Create byte array and return
                    match env.byte_array_from_slice(&data) {
                        Ok(arr) => return arr.into_raw(),
                        Err(e) => {
                            log::warn!("Failed to create byte array: {:?}", e);
                        }
                    }
                }
            }
        }
    }
    
    // Return null on failure
    std::ptr::null_mut()
}

/// JNI callback: Restore native state
/// Java signature: private native void restoreNativeState(byte[] state);
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_ratadroid_template_NativeActivity_restoreNativeState(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JObject<'_>,
    state: jni::sys::jbyteArray,
) {
    use jni::objects::JByteArray;
    
    if state.is_null() {
        return;
    }
    
    if let Ok(guard) = GLOBAL_STATE.lock() {
        if let Some(AppStatePtr(state_ptr)) = guard.as_ref() {
            unsafe {
                let app_state = &mut **state_ptr;
                
                // Read byte array
                let byte_array = JByteArray::from_raw(state);
                if let Ok(len) = env.get_array_length(&byte_array) {
                    let mut data = vec![0i8; len as usize];
                    if env.get_byte_array_region(&byte_array, 0, &mut data).is_ok() {
                        // Convert signed bytes to unsigned
                        let unsigned_data: Vec<u8> = data.iter().map(|&b| b as u8).collect();
                        restore_state(app_state, &unsigned_data);
                    }
                }
            }
        }
    }
}

