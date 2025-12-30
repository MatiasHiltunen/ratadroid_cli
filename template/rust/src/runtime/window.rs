//! Window lifecycle management for Android NativeActivity

use crate::Orientation;
use jni::objects::JObject;
use log::info;
use ndk::native_window::NativeWindow;
use ndk_sys::ANativeWindow_setBuffersGeometry;

/// Window manager that tracks window state and handles lifecycle events
pub struct WindowManager {
    /// Current native window (None if window is destroyed)
    pub native_window: Option<NativeWindow>,
    /// Window width in pixels
    pub width_px: u32,
    /// Window height in pixels
    pub height_px: u32,
    /// Visible height (accounting for keyboard)
    pub visible_height_px: u32,
    /// Current orientation
    pub orientation: Orientation,
    /// Status bar height in pixels
    pub status_bar_height_px: u32,
    /// Navigation bar height in pixels
    pub nav_bar_height_px: u32,
}

impl WindowManager {
    /// Create a new window manager
    pub fn new() -> Self {
        Self {
            native_window: None,
            width_px: 0,
            height_px: 0,
            visible_height_px: 0,
            orientation: Orientation::Portrait,
            status_bar_height_px: 48,
            nav_bar_height_px: 48,
        }
    }

    /// Initialize window when created
    pub fn init_window(&mut self, window: &NativeWindow) -> Result<(), String> {
        let width = window.width();
        let height = window.height();
        
        self.width_px = width as u32;
        self.height_px = height as u32;
        self.visible_height_px = height as u32;
        self.orientation = if width > height {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };
        
        // Get screen insets
        let (status_bar, nav_bar) = get_screen_insets();
        self.status_bar_height_px = status_bar;
        self.nav_bar_height_px = nav_bar;
        
        // Set buffer geometry
        unsafe {
            let native_window_ptr = window.ptr().as_ptr() as *mut ndk_sys::ANativeWindow;
            ANativeWindow_setBuffersGeometry(native_window_ptr, width as i32, height as i32, 1);
        }
        
        self.native_window = Some(window.clone());
        info!("Window initialized: {}x{}px, orientation: {:?}", width, height, self.orientation);
        
        Ok(())
    }

    /// Handle window resize
    pub fn handle_resize(&mut self, width: i32, height: i32) {
        self.width_px = width as u32;
        self.height_px = height as u32;
        
        // Update visible height if not set by keyboard
        if self.visible_height_px == 0 {
            self.visible_height_px = height as u32;
        }
        
        let new_orientation = if width > height {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };
        
        if self.orientation != new_orientation {
            self.orientation = new_orientation;
            info!("Orientation changed to {:?}", new_orientation);
        }
        
        // Update buffer geometry
        if let Some(ref window) = self.native_window {
            unsafe {
                let native_window_ptr = window.ptr().as_ptr() as *mut ndk_sys::ANativeWindow;
                ANativeWindow_setBuffersGeometry(native_window_ptr, width, height, 1);
            }
        }
        
        info!("Window resized: {}x{}px", width, height);
    }

    /// Update visible height (when keyboard shows/hides)
    pub fn update_visible_height(&mut self, visible_height: u32) {
        if self.visible_height_px != visible_height {
            let old = self.visible_height_px;
            self.visible_height_px = visible_height;
            info!("Visible height updated: {} -> {}px", old, visible_height);
        }
    }

    /// Terminate window (cleanup when window is destroyed)
    pub fn term_window(&mut self) {
        info!("Window terminated, cleaning up");
        self.native_window = None;
        // Note: Don't reset dimensions as they may be needed for state save
    }

    /// Check if window is valid
    pub fn is_valid(&self) -> bool {
        self.native_window.is_some()
    }

    /// Get window reference (if valid)
    pub fn get_window(&self) -> Option<&NativeWindow> {
        self.native_window.as_ref()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get screen insets (status bar, navigation bar) via direct JNI
fn get_screen_insets() -> (u32, u32) {
    use ndk_context::android_context;
    use jni::objects::JObject;
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    let activity_obj = ctx.context();
    
    if vm_ptr.is_null() || activity_obj.is_null() {
        return (48, 48); // Fallback to default values
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
        
        let activity_jobj = JObject::from_raw(activity_obj as jni::sys::jobject);
        
        // Try to get WindowInsets (API 30+)
        if let Ok((status_bar, nav_bar)) = get_insets_via_window_insets(&mut env, &activity_jobj) {
            return (status_bar, nav_bar);
        }
        
        // Fallback: Use resource identifiers
        if let Ok((status_bar, nav_bar)) = get_insets_via_resources(&mut env, &activity_jobj) {
            return (status_bar, nav_bar);
        }
        
        // Final fallback
        (48, 48)
    }
}

/// Get insets via WindowInsets API (Android 11+ / API 30+)
fn get_insets_via_window_insets(env: &mut jni::JNIEnv, activity_obj: &JObject) -> Result<(u32, u32), String> {
    // Get window decor view
    let window_result = env.call_method(activity_obj, "getWindow", "()Landroid/view/Window;", &[]);
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Err("Exception getting window".to_string());
    }
    
    let window_obj = window_result
        .map_err(|e| format!("Failed to get window: {:?}", e))?
        .l()
        .map_err(|e| format!("Failed to convert window: {:?}", e))?;
    
    if window_obj.is_null() {
        return Err("Window is null".to_string());
    }
    
    let decor_view_result = env.call_method(&window_obj, "getDecorView", "()Landroid/view/View;", &[]);
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Err("Exception getting decor view".to_string());
    }
    
    let decor_view_obj = decor_view_result
        .map_err(|e| format!("Failed to get decor view: {:?}", e))?
        .l()
        .map_err(|e| format!("Failed to convert decor view: {:?}", e))?;
    
    if decor_view_obj.is_null() {
        return Err("Decor view is null".to_string());
    }
    
    // Get root window insets
    let insets_result = env.call_method(&decor_view_obj, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[]);
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Err("Exception getting window insets".to_string());
    }
    
    let insets_obj = insets_result
        .map_err(|e| format!("Failed to get window insets: {:?}", e))?
        .l()
        .map_err(|e| format!("Failed to convert insets: {:?}", e))?;
    
    if insets_obj.is_null() {
        return Err("Window insets is null".to_string());
    }
    
    // Check API level - use appropriate method
    let build_version_class = env.find_class("android/os/Build$VERSION")
        .map_err(|e| format!("Failed to find Build.VERSION: {:?}", e))?;
    
    let sdk_int_result = env.get_static_field(&build_version_class, "SDK_INT", "I");
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Err("Exception getting SDK_INT".to_string());
    }
    
    let sdk_int = sdk_int_result
        .map_err(|e| format!("Failed to get SDK_INT: {:?}", e))?
        .i()
        .map_err(|e| format!("Failed to convert SDK_INT: {:?}", e))?;
    
    let (top, bottom) = if sdk_int >= 30 {
        // API 30+: Use getInsets(Type.systemBars())
        let type_class = env.find_class("android/view/WindowInsets$Type")
            .map_err(|e| format!("Failed to find WindowInsets.Type: {:?}", e))?;
        
        let system_bars_result = env.call_static_method(&type_class, "systemBars", "()Landroid/view/WindowInsets$Type;", &[]);
        if env.exception_check().unwrap_or(false) {
            env.exception_clear().ok();
            return Err("Exception getting systemBars type".to_string());
        }
        
        let system_bars_type = system_bars_result
            .map_err(|e| format!("Failed to get systemBars type: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert systemBars type: {:?}", e))?;
        
        let insets_result = env.call_method(&insets_obj, "getInsets", "(Landroid/view/WindowInsets$Type;)Landroid/graphics/Insets;", &[(&system_bars_type).into()]);
        if env.exception_check().unwrap_or(false) {
            env.exception_clear().ok();
            return Err("Exception getting insets".to_string());
        }
        
        let insets_rect = insets_result
            .map_err(|e| format!("Failed to get insets: {:?}", e))?
            .l()
            .map_err(|e| format!("Failed to convert insets: {:?}", e))?;
        
        let top_result = env.call_method(&insets_rect, "top", "()I", &[]);
        let bottom_result = env.call_method(&insets_rect, "bottom", "()I", &[]);
        
        if env.exception_check().unwrap_or(false) {
            env.exception_clear().ok();
            return Err("Exception getting inset values".to_string());
        }
        
        let top = top_result
            .map_err(|e| format!("Failed to get top: {:?}", e))?
            .i()
            .map_err(|e| format!("Failed to convert top: {:?}", e))?;
        
        let bottom = bottom_result
            .map_err(|e| format!("Failed to get bottom: {:?}", e))?
            .i()
            .map_err(|e| format!("Failed to convert bottom: {:?}", e))?;
        
        (top.max(0) as u32, bottom.max(0) as u32)
    } else {
        // API < 30: Use getSystemWindowInsets()
        let top_result = env.call_method(&insets_obj, "getSystemWindowInsetTop", "()I", &[]);
        let bottom_result = env.call_method(&insets_obj, "getSystemWindowInsetBottom", "()I", &[]);
        
        if env.exception_check().unwrap_or(false) {
            env.exception_clear().ok();
            return Err("Exception getting system window insets".to_string());
        }
        
        let top = top_result
            .map_err(|e| format!("Failed to get top: {:?}", e))?
            .i()
            .map_err(|e| format!("Failed to convert top: {:?}", e))?;
        
        let bottom = bottom_result
            .map_err(|e| format!("Failed to get bottom: {:?}", e))?
            .i()
            .map_err(|e| format!("Failed to convert bottom: {:?}", e))?;
        
        (top.max(0) as u32, bottom.max(0) as u32)
    };
    
    Ok((top, bottom))
}

/// Get insets via resource identifiers (fallback for older APIs)
fn get_insets_via_resources(env: &mut jni::JNIEnv, activity_obj: &JObject) -> Result<(u32, u32), String> {
    // Get Resources
    let resources_result = env.call_method(activity_obj, "getResources", "()Landroid/content/res/Resources;", &[]);
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Err("Exception getting resources".to_string());
    }
    
    let resources_obj = resources_result
        .map_err(|e| format!("Failed to get resources: {:?}", e))?
        .l()
        .map_err(|e| format!("Failed to convert resources: {:?}", e))?;
    
    if resources_obj.is_null() {
        return Err("Resources is null".to_string());
    }
    
    // Get status bar height
    let status_bar_name = env.new_string("status_bar_height")
        .map_err(|e| format!("Failed to create status_bar_height string: {:?}", e))?;
    let dimen_name = env.new_string("dimen")
        .map_err(|e| format!("Failed to create dimen string: {:?}", e))?;
    let android_name = env.new_string("android")
        .map_err(|e| format!("Failed to create android string: {:?}", e))?;
    
    let status_bar_id_result = env.call_method(
        &resources_obj,
        "getIdentifier",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
        &[(&status_bar_name).into(), (&dimen_name).into(), (&android_name).into()]
    );
    
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Err("Exception getting status_bar_height identifier".to_string());
    }
    
    let status_bar_id = status_bar_id_result
        .map_err(|e| format!("Failed to get status_bar_height id: {:?}", e))?
        .i()
        .map_err(|e| format!("Failed to convert status_bar_height id: {:?}", e))?;
    
    let status_bar_height = if status_bar_id > 0 {
        let dimen_result = env.call_method(&resources_obj, "getDimensionPixelSize", "(I)I", &[status_bar_id.into()]);
        if env.exception_check().unwrap_or(false) {
            env.exception_clear().ok();
            48u32 // Fallback
        } else {
            dimen_result
                .map_err(|_| "Failed to get dimension".to_string())?
                .i()
                .map_err(|_| "Failed to convert dimension".to_string())?
                .max(0) as u32
        }
    } else {
        48u32 // Fallback
    };
    
    // Get navigation bar height
    let nav_bar_name = env.new_string("navigation_bar_height")
        .map_err(|e| format!("Failed to create navigation_bar_height string: {:?}", e))?;
    
    let nav_bar_id_result = env.call_method(
        &resources_obj,
        "getIdentifier",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
        &[(&nav_bar_name).into(), (&dimen_name).into(), (&android_name).into()]
    );
    
    if env.exception_check().unwrap_or(false) {
        env.exception_clear().ok();
        return Ok((status_bar_height, 48));
    }
    
    let nav_bar_id = nav_bar_id_result
        .map_err(|e| format!("Failed to get navigation_bar_height id: {:?}", e))?
        .i()
        .map_err(|e| format!("Failed to convert navigation_bar_height id: {:?}", e))?;
    
    let nav_bar_height = if nav_bar_id > 0 {
        let dimen_result = env.call_method(&resources_obj, "getDimensionPixelSize", "(I)I", &[nav_bar_id.into()]);
        if env.exception_check().unwrap_or(false) {
            env.exception_clear().ok();
            48u32 // Fallback
        } else {
            dimen_result
                .map_err(|_| "Failed to get dimension".to_string())?
                .i()
                .map_err(|_| "Failed to convert dimension".to_string())?
                .max(0) as u32
        }
    } else {
        48u32 // Fallback
    };
    
    Ok((status_bar_height, nav_bar_height))
}

