//! Keyboard management for Android soft keyboard and visibility tracking

use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Global atomic flag to signal keyboard visibility changed
static KEYBOARD_VISIBILITY_CHANGED: AtomicBool = AtomicBool::new(false);

/// Global atomic for visible height when keyboard visibility changes
static KEYBOARD_VISIBLE_HEIGHT: AtomicU32 = AtomicU32::new(0);

/// Track if the soft keyboard is currently visible
static SOFT_KEYBOARD_VISIBLE: AtomicBool = AtomicBool::new(false);

/// Previous window height for keyboard detection
static PREVIOUS_WINDOW_HEIGHT: AtomicU32 = AtomicU32::new(0);

/// Detect keyboard visibility based on window size changes
/// Compares current window height with previous height
/// If height decreased by >15%, keyboard is likely visible
/// Also corrects stale state from programmatic hide/show operations
pub fn detect_keyboard_visibility(current_height: u32, screen_height: u32) -> bool {
    let prev_height = PREVIOUS_WINDOW_HEIGHT.load(Ordering::SeqCst);
    let current_state = SOFT_KEYBOARD_VISIBLE.load(Ordering::SeqCst);
    
    // Initialize on first call
    if prev_height == 0 {
        PREVIOUS_WINDOW_HEIGHT.store(current_height, Ordering::SeqCst);
        KEYBOARD_VISIBLE_HEIGHT.store(current_height, Ordering::SeqCst);
        SOFT_KEYBOARD_VISIBLE.store(false, Ordering::SeqCst);
        return false;
    }
    
    // Check for significant height change (>15% threshold)
    let height_diff = if current_height < prev_height {
        prev_height - current_height
    } else {
        current_height - prev_height
    };
    
    // Use 15% threshold like Java implementation
    let threshold = (screen_height as f32 * 0.15) as u32;
    
    if height_diff > threshold.max(100) {
        // Significant change detected - keyboard visibility changed
        let was_visible = current_state;
        let is_visible = current_height < prev_height;
        
        if was_visible != is_visible {
            info!("Keyboard visibility detected via window resize: {} -> {} (height: {} -> {}px, diff: {}px)",
                if was_visible { "VISIBLE" } else { "HIDDEN" },
                if is_visible { "VISIBLE" } else { "HIDDEN" },
                prev_height, current_height, height_diff);
            
            KEYBOARD_VISIBLE_HEIGHT.store(current_height, Ordering::SeqCst);
            SOFT_KEYBOARD_VISIBLE.store(is_visible, Ordering::SeqCst);
            KEYBOARD_VISIBILITY_CHANGED.store(true, Ordering::SeqCst);
        }
        
        PREVIOUS_WINDOW_HEIGHT.store(current_height, Ordering::SeqCst);
        return is_visible;
    }
    
    // No significant change, but verify state consistency
    // If height is close to screen height and we think keyboard is visible, it's likely hidden
    // If height is significantly less than screen height and we think keyboard is hidden, it might be visible
    let height_ratio = current_height as f32 / screen_height as f32;
    let likely_visible = height_ratio < 0.85; // Less than 85% of screen height suggests keyboard
    
    if current_state != likely_visible && height_diff > 50 {
        // State mismatch detected - correct it
        warn!("Keyboard state mismatch corrected: state={}, likely_visible={}, height={}px (screen={}px, ratio={:.2})",
            if current_state { "VISIBLE" } else { "HIDDEN" },
            if likely_visible { "VISIBLE" } else { "HIDDEN" },
            current_height, screen_height, height_ratio);
        
        SOFT_KEYBOARD_VISIBLE.store(likely_visible, Ordering::SeqCst);
        KEYBOARD_VISIBLE_HEIGHT.store(current_height, Ordering::SeqCst);
        KEYBOARD_VISIBILITY_CHANGED.store(true, Ordering::SeqCst);
    }
    
    // Update previous height even if no keyboard change
    PREVIOUS_WINDOW_HEIGHT.store(current_height, Ordering::SeqCst);
    SOFT_KEYBOARD_VISIBLE.load(Ordering::SeqCst)
}

/// Check if keyboard visibility has changed since last check
pub fn check_keyboard_visibility_changed() -> bool {
    KEYBOARD_VISIBILITY_CHANGED.swap(false, Ordering::SeqCst)
}

/// Get the current visible height (accounting for keyboard)
pub fn get_visible_height() -> u32 {
    KEYBOARD_VISIBLE_HEIGHT.load(Ordering::SeqCst)
}

/// Check if the Android soft keyboard is currently visible
pub fn is_soft_keyboard_visible() -> bool {
    SOFT_KEYBOARD_VISIBLE.load(Ordering::SeqCst)
}

/// Set keyboard visibility state directly (for programmatic show/hide)
/// This updates the state optimistically when we programmatically show/hide the keyboard
/// Window resize events will still correct the state if needed
fn set_keyboard_visible(visible: bool, current_height: u32) {
    let was_visible = SOFT_KEYBOARD_VISIBLE.swap(visible, Ordering::SeqCst);
    
    if was_visible != visible {
        info!("Keyboard state updated programmatically: {} -> {} (height: {}px)",
            if was_visible { "VISIBLE" } else { "HIDDEN" },
            if visible { "VISIBLE" } else { "HIDDEN" },
            current_height);
        
        KEYBOARD_VISIBLE_HEIGHT.store(current_height, Ordering::SeqCst);
        KEYBOARD_VISIBILITY_CHANGED.store(true, Ordering::SeqCst);
        
        // Update previous height to prevent false positives in detection
        // Use current height as baseline for future comparisons
        PREVIOUS_WINDOW_HEIGHT.store(current_height, Ordering::SeqCst);
    }
}

/// Set keyboard to visible state
/// Uses stored height as reference (will be corrected by window resize events)
fn set_keyboard_visible_state() {
    // Use stored visible height or previous window height as reference
    let current_height = KEYBOARD_VISIBLE_HEIGHT.load(Ordering::SeqCst)
        .max(PREVIOUS_WINDOW_HEIGHT.load(Ordering::SeqCst));
    
    if current_height > 0 {
        set_keyboard_visible(true, current_height);
    } else {
        // Fallback: just update the flag if we don't have height info yet
        let was_visible = SOFT_KEYBOARD_VISIBLE.swap(true, Ordering::SeqCst);
        if !was_visible {
            info!("Keyboard state updated programmatically: HIDDEN -> VISIBLE (height unknown)");
            KEYBOARD_VISIBILITY_CHANGED.store(true, Ordering::SeqCst);
        }
    }
}

/// Set keyboard to hidden state
/// Uses stored height as reference (will be corrected by window resize events)
fn set_keyboard_hidden_state() {
    // Use stored visible height or previous window height as reference
    let current_height = KEYBOARD_VISIBLE_HEIGHT.load(Ordering::SeqCst)
        .max(PREVIOUS_WINDOW_HEIGHT.load(Ordering::SeqCst));
    
    if current_height > 0 {
        set_keyboard_visible(false, current_height);
    } else {
        // Fallback: just update the flag if we don't have height info yet
        let was_visible = SOFT_KEYBOARD_VISIBLE.swap(false, Ordering::SeqCst);
        if was_visible {
            info!("Keyboard state updated programmatically: VISIBLE -> HIDDEN (height unknown)");
            KEYBOARD_VISIBILITY_CHANGED.store(true, Ordering::SeqCst);
        }
    }
}

/// Show the Android soft keyboard via JNI
pub fn show_soft_keyboard() {
    let current_state = is_soft_keyboard_visible();
    info!("Attempting to show soft keyboard (current state: {})...", 
        if current_state { "VISIBLE" } else { "HIDDEN" });
    show_keyboard_via_jni();
    let new_state = is_soft_keyboard_visible();
    if new_state != current_state {
        info!("Keyboard show completed: state changed {} -> {}", 
            if current_state { "VISIBLE" } else { "HIDDEN" },
            if new_state { "VISIBLE" } else { "HIDDEN" });
    }
}

/// Hide the Android soft keyboard via JNI
pub fn hide_soft_keyboard() {
    let current_state = is_soft_keyboard_visible();
    info!("Attempting to hide soft keyboard (current state: {})...", 
        if current_state { "VISIBLE" } else { "HIDDEN" });
    hide_keyboard_via_jni();
    let new_state = is_soft_keyboard_visible();
    if new_state != current_state {
        info!("Keyboard hide completed: state changed {} -> {}", 
            if current_state { "VISIBLE" } else { "HIDDEN" },
            if new_state { "VISIBLE" } else { "HIDDEN" });
    } else if current_state {
        warn!("Keyboard hide called but state still shows VISIBLE - may need window resize to sync");
    }
}

/// Internal: Show keyboard via direct JNI call to InputMethodManager
fn show_keyboard_via_jni() {
    match show_keyboard_direct() {
        Ok(_) => {
            // Optimistically update state after successful JNI call
            set_keyboard_visible_state();
        }
        Err(e) => {
            error!("Failed to show keyboard: {:?}", e);
        }
    }
}

/// Internal: Hide keyboard via direct JNI call to InputMethodManager
fn hide_keyboard_via_jni() {
    match hide_keyboard_direct() {
        Ok(_) => {
            // Optimistically update state after successful JNI call
            set_keyboard_hidden_state();
        }
        Err(e) => {
            error!("Failed to hide keyboard: {:?}", e);
        }
    }
}

/// Show keyboard directly via InputMethodManager (called on UI thread)
fn show_keyboard_direct() -> Result<(), String> {
    use ndk_context::android_context;
    use jni::objects::JObject;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let activity_obj = ctx.context();
    
    if vm_ptr.is_null() || activity_obj.is_null() {
        return Err("VM or activity object is null".to_string());
    }
    
    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr as *mut _)
            .map_err(|e| format!("Failed to create JavaVM: {:?}", e))?;
        
        let mut env = vm.attach_current_thread()
            .map_err(|e| format!("Failed to attach thread: {:?}", e))?;
        
        let activity_jobj = JObject::from_raw(activity_obj as jni::sys::jobject);
        
        // Get window decor view
        let window_result = env.call_method(&activity_jobj, "getWindow", "()Landroid/view/Window;", &[]);
        if handle_jni_exception(&mut env, "getWindow") {
            return Err("Exception getting window".to_string());
        }
        
        let window_obj = window_result
            .map_err(|e| format!("Failed to get window: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert window to object: {:?}", e))?;
        
        if window_obj.is_null() {
            return Err("Window object is null".to_string());
        }
        
        let decor_view_result = env.call_method(&window_obj, "getDecorView", "()Landroid/view/View;", &[]);
        if handle_jni_exception(&mut env, "getDecorView") {
            return Err("Exception getting decor view".to_string());
        }
        
        let decor_view_obj = decor_view_result
            .map_err(|e| format!("Failed to get decor view: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert decor view to object: {:?}", e))?;
        
        if decor_view_obj.is_null() {
            return Err("Decor view is null".to_string());
        }
        
        // Request focus on decor view
        let focus_result = env.call_method(&decor_view_obj, "requestFocus", "()Z", &[]);
        if handle_jni_exception(&mut env, "requestFocus") {
            return Err("Exception requesting focus".to_string());
        }
        let _ = focus_result.ok();
        
        // Get InputMethodManager service
        let service_name = env.new_string("input_method")
            .map_err(|e| format!("Failed to create string: {:?}", e))?;
        
        let imm_result = env.call_method(
            &activity_jobj,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[(&service_name).into()]
        );
        
        if handle_jni_exception(&mut env, "getSystemService") {
            return Err("Exception getting system service".to_string());
        }
        
        let imm_obj = imm_result
            .map_err(|e| format!("Failed to get InputMethodManager: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert to object: {:?}", e))?;
        
        if imm_obj.is_null() {
            return Err("InputMethodManager is null".to_string());
        }
        
        // Show keyboard - SHOW_IMPLICIT = 1 (easy to dismiss)
        let show_result = env.call_method(
            &imm_obj,
            "showSoftInput",
            "(Landroid/view/View;I)Z",
            &[(&decor_view_obj).into(), 1i32.into()]
        );
        
        if handle_jni_exception(&mut env, "showSoftInput") {
            return Err("Exception calling showSoftInput".to_string());
        }
        
        let shown = show_result
            .map_err(|e| format!("showSoftInput failed: {:?}", e))?
            .z()
            .map_err(|e| format!("Failed to convert result to boolean: {:?}", e))?;
        
        if !shown {
            // Fallback: toggle on
            let toggle_result = env.call_method(
                &imm_obj,
                "toggleSoftInput",
                "(II)V",
                &[1i32.into(), 0i32.into()]
            );
            
            if handle_jni_exception(&mut env, "toggleSoftInput") {
                return Err("Exception calling toggleSoftInput".to_string());
            }
            
            toggle_result.map_err(|e| format!("toggleSoftInput failed: {:?}", e))?;
            info!("Keyboard shown via toggleSoftInput fallback");
        } else {
            info!("Keyboard shown via showSoftInput");
        }
        
        Ok(())
    }
}

/// Hide keyboard directly via InputMethodManager (called on UI thread)
fn hide_keyboard_direct() -> Result<(), String> {
    use ndk_context::android_context;
    use jni::objects::JObject;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let activity_obj = ctx.context();
    
    if vm_ptr.is_null() || activity_obj.is_null() {
        return Err("VM or activity object is null".to_string());
    }
    
    unsafe {
        let vm = jni::JavaVM::from_raw(vm_ptr as *mut _)
            .map_err(|e| format!("Failed to create JavaVM: {:?}", e))?;
        
        let mut env = vm.attach_current_thread()
            .map_err(|e| format!("Failed to attach thread: {:?}", e))?;
        
        let activity_jobj = JObject::from_raw(activity_obj as jni::sys::jobject);
        
        // Get window decor view
        let window_result = env.call_method(&activity_jobj, "getWindow", "()Landroid/view/Window;", &[]);
        if handle_jni_exception(&mut env, "getWindow") {
            return Err("Exception getting window".to_string());
        }
        
        let window_obj = window_result
            .map_err(|e| format!("Failed to get window: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert window to object: {:?}", e))?;
        
        if window_obj.is_null() {
            return Err("Window object is null".to_string());
        }
        
        let decor_view_result = env.call_method(&window_obj, "getDecorView", "()Landroid/view/View;", &[]);
        if handle_jni_exception(&mut env, "getDecorView") {
            return Err("Exception getting decor view".to_string());
        }
        
        let decor_view_obj = decor_view_result
            .map_err(|e| format!("Failed to get decor view: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert decor view to object: {:?}", e))?;
        
        if decor_view_obj.is_null() {
            return Err("Decor view is null".to_string());
        }
        
        // Clear focus
        let clear_focus_result = env.call_method(&decor_view_obj, "clearFocus", "()V", &[]);
        if handle_jni_exception(&mut env, "clearFocus") {
            // Non-critical, continue
        }
        let _ = clear_focus_result.ok();
        
        // Get window token
        let token_result = env.call_method(&decor_view_obj, "getWindowToken", "()Landroid/os/IBinder;", &[]);
        if handle_jni_exception(&mut env, "getWindowToken") {
            return Err("Exception getting window token".to_string());
        }
        
        let token_obj = token_result
            .map_err(|e| format!("Failed to get window token: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert token to object: {:?}", e))?;
        
        if token_obj.is_null() {
            return Err("Window token is null".to_string());
        }
        
        // Get InputMethodManager service
        let service_name = env.new_string("input_method")
            .map_err(|e| format!("Failed to create string: {:?}", e))?;
        
        let imm_result = env.call_method(
            &activity_jobj,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[(&service_name).into()]
        );
        
        if handle_jni_exception(&mut env, "getSystemService") {
            return Err("Exception getting system service".to_string());
        }
        
        let imm_obj = imm_result
            .map_err(|e| format!("Failed to get InputMethodManager: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert to object: {:?}", e))?;
        
        if imm_obj.is_null() {
            return Err("InputMethodManager is null".to_string());
        }
        
        // Hide keyboard via window token
        let hide_result = env.call_method(
            &imm_obj,
            "hideSoftInputFromWindow",
            "(Landroid/os/IBinder;I)Z",
            &[(&token_obj).into(), 0i32.into()]
        );
        
        if handle_jni_exception(&mut env, "hideSoftInputFromWindow") {
            return Err("Exception calling hideSoftInputFromWindow".to_string());
        }
        
        let hidden = hide_result
            .map_err(|e| format!("hideSoftInputFromWindow failed: {:?}", e))?
            .z()
            .map_err(|e| format!("Failed to convert result to boolean: {:?}", e))?;
        
        if !hidden {
            // Fallback: toggle off
            let toggle_result = env.call_method(
                &imm_obj,
                "toggleSoftInput",
                "(II)V",
                &[0i32.into(), 1i32.into()]
            );
            
            if handle_jni_exception(&mut env, "toggleSoftInput") {
                return Err("Exception calling toggleSoftInput".to_string());
            }
            
            toggle_result.map_err(|e| format!("toggleSoftInput failed: {:?}", e))?;
            info!("Keyboard hidden via toggleSoftInput fallback");
        } else {
            info!("Keyboard hidden via hideSoftInputFromWindow");
        }
        
        Ok(())
    }
}

/// Handle JNI exceptions
fn handle_jni_exception(env: &mut jni::JNIEnv, context: &str) -> bool {
    if env.exception_check().unwrap_or(false) {
        if let Err(desc_err) = env.exception_describe() {
            warn!("Failed to describe exception in {}: {:?}", context, desc_err);
        } else {
            error!("JNI exception in {}", context);
        }
        env.exception_clear().ok();
        return true;
    }
    false
}


