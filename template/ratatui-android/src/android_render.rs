//! Android native text rendering via JNI
//!
//! This module provides Android Canvas/Bitmap rendering as a fallback
//! for emojis when cosmic-text fails. It uses Android's native rendering
//! APIs which provide superior emoji quality.

#[cfg(all(target_os = "android", feature = "android-native-render"))]
use crate::rasterizer::{CachedChar, CHAR_CACHE, is_wide_char};
#[cfg(all(target_os = "android", feature = "android-native-render"))]
use jni::objects::{JObject, JString, JByteArray};
#[cfg(all(target_os = "android", feature = "android-native-render"))]
use jni::sys::jobject;
#[cfg(all(target_os = "android", feature = "android-native-render"))]
use jni::{JavaVM, JNIEnv};
#[cfg(all(target_os = "android", feature = "android-native-render"))]
use log::{error, warn};
#[cfg(all(target_os = "android", feature = "android-native-render"))]
use ndk_context::android_context;
#[cfg(all(target_os = "android", feature = "android-native-render"))]
use std::sync::OnceLock;

#[cfg(all(target_os = "android", feature = "android-native-render"))]
static JAVA_VM_CACHE: OnceLock<Option<JavaVM>> = OnceLock::new();

#[cfg(all(target_os = "android", feature = "android-native-render"))]
fn get_java_vm() -> Result<&'static JavaVM, String> {
    if let Some(vm) = JAVA_VM_CACHE.get().and_then(|v| v.as_ref()) {
        return Ok(vm);
    }
    
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    
    if vm_ptr.is_null() {
        return Err("JavaVM pointer is null".to_string());
    }
    
    unsafe {
        let vm = JavaVM::from_raw(vm_ptr as *mut _)
            .map_err(|e| format!("Failed to create JavaVM: {:?}", e))?;
        
        JAVA_VM_CACHE.set(Some(vm))
            .map_err(|_| "JavaVM already initialized".to_string())?;
    }
    
    JAVA_VM_CACHE
        .get()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| "Failed to get JavaVM".to_string())
}

#[cfg(all(target_os = "android", feature = "android-native-render"))]
fn get_android_context() -> Result<jobject, String> {
    let ctx = android_context();
    let context_obj = ctx.context();
    
    if context_obj.is_null() {
        return Err("Android context is null".to_string());
    }
    
    Ok(context_obj as jobject)
}

#[cfg(all(target_os = "android", feature = "android-native-render"))]
fn handle_jni_exception(env: &mut JNIEnv, context: &str) -> bool {
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

/// Render a character using Android's native Canvas/Bitmap APIs.
/// This provides superior emoji rendering quality compared to cosmic-text.
/// 
/// Returns (width, height, is_wide, rgba_data) if successful.
#[cfg(all(target_os = "android", feature = "android-native-render"))]
pub fn render_char_android(
    c: char,
    size: f32,
    color: [u8; 4],
) -> Option<CachedChar> {
    // Create cache key (same format as cosmic-text)
    // Note: We use a different cache key prefix to avoid conflicts with cosmic-text cache
    let size_u32 = size as u32;
    let color_u32 =
        ((color[3] as u32) << 24) | ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | (color[2] as u32);
    // Use a special marker in the cache key to distinguish Android-rendered emojis
    // We'll use char_code | 0x10000000 as a marker (bit 28 set)
    let cache_key = (c as u32 | 0x10000000, size_u32, color_u32);

    // Try to get from cache first
    {
        let mut cache = match CHAR_CACHE.lock() {
            Ok(cache) => cache,
            Err(e) => {
                warn!("Failed to lock character cache: {:?}", e);
                return None;
            }
        };
        if let Some(cached) = cache.get(&cache_key) {
            return Some((cached.0, cached.1, cached.2, cached.3.clone()));
        }
    }

    // Log first Android render attempt for debugging
    static FIRST_ANDROID_RENDER_LOGGED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    let should_log = !FIRST_ANDROID_RENDER_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed);
    
    if should_log {
        log::info!(
            "First Android render attempt: char='{}' (U+{:04X}), size={}, color=RGBA({},{},{},{})",
            c,
            c as u32,
            size,
            color[0],
            color[1],
            color[2],
            color[3]
        );
    }

    // Get JavaVM and attach thread
    let vm = match get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            warn!("Failed to get JavaVM for Android rendering: {}", e);
            return None;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            warn!("Failed to attach thread for Android rendering: {:?}", e);
            return None;
        }
    };

    // Get Android context
    let context_obj = match get_android_context() {
        Ok(obj) => obj,
        Err(e) => {
            warn!("Failed to get Android context: {}", e);
            return None;
        }
    };

    let activity_jobj = unsafe { JObject::from_raw(context_obj) };

    // Convert character to Java String
    let char_str = c.to_string();
    let jstring = match env.new_string(&char_str) {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to create Java string: {:?}", e);
            return None;
        }
    };

    // Convert color to ARGB format (Android uses ARGB, we have RGBA)
    // Android Paint.setColor expects ARGB: 0xAARRGGBB
    let argb_color = ((color[3] as i32) << 24)
        | ((color[0] as i32) << 16)
        | ((color[1] as i32) << 8)
        | (color[2] as i32);
    
    if should_log {
        log::info!("Calling Java renderCharacter with ARGB color: 0x{:08X}", argb_color);
    }

    // Call Java method: renderCharacter(String character, float size, int color) -> byte[]
    let result = env.call_method(
        &activity_jobj,
        "renderCharacter",
        "(Ljava/lang/String;FI)[B",
        &[
            jni::objects::JValue::Object(&jstring),
            jni::objects::JValue::Float(size),
            jni::objects::JValue::Int(argb_color),
        ],
    );

    // Check for exceptions
    if handle_jni_exception(&mut env, "renderCharacter") {
        return None;
    }

    // Extract byte array
    let byte_array = match result {
        Ok(val) => val,
        Err(e) => {
            warn!("Failed to call renderCharacter: {:?}", e);
            return None;
        }
    };

    let array_obj = match byte_array.l() {
        Ok(obj) => obj,
        Err(e) => {
            warn!("Failed to convert to object: {:?}", e);
            return None;
        }
    };

    let byte_array: JByteArray = unsafe { JByteArray::from_raw(array_obj.into_raw()) };
    let len = match env.get_array_length(&byte_array) {
        Ok(l) => l as usize,
        Err(e) => {
            warn!("Failed to get array length: {:?}", e);
            return None;
        }
    };

    // Empty array means failure
    if len < 9 {
        return None;
    }

    // Read byte array (JNI uses signed bytes)
    let mut signed_bytes = vec![0i8; len];
    match env.get_byte_array_region(&byte_array, 0, &mut signed_bytes) {
        Ok(_) => {}
        Err(e) => {
            warn!("Failed to read byte array: {:?}", e);
            return None;
        }
    }

    // Convert signed bytes to unsigned
    let bytes: Vec<u8> = signed_bytes.iter().map(|&b| b as u8).collect();

    // Parse result: [width(4), height(4), isWide(1), ...rgba_pixels]
    let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let is_wide = bytes[8] != 0;

    if should_log {
        log::info!(
            "Android render result: width={}, height={}, is_wide={}, total_bytes={}",
            width,
            height,
            is_wide,
            len
        );
    }

    // Extract RGBA pixel data (starting at offset 9)
    if len < 9 + (width * height * 4) as usize {
        warn!(
            "Byte array too short for pixel data: len={}, expected_at_least={}",
            len,
            9 + (width * height * 4) as usize
        );
        return None;
    }

    let rgba_data = bytes[9..].to_vec();

    // Verify dimensions match
    let expected_pixels = (width * height * 4) as usize;
    if rgba_data.len() < expected_pixels {
        warn!(
            "Pixel data length mismatch: expected {}, got {}",
            expected_pixels,
            rgba_data.len()
        );
        return None;
    }

    // Count non-transparent pixels for debugging
    if should_log {
        let non_transparent = rgba_data
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        log::info!("Android render: {} non-transparent pixels out of {}", non_transparent, expected_pixels / 4);
    }

    let result = (width, height, is_wide, rgba_data);

    // Cache the result
    if let Ok(mut cache) = CHAR_CACHE.lock() {
        cache.put(cache_key, result.clone());
    }

    Some(result)
}

/// Stub implementation when feature is disabled
#[cfg(not(all(target_os = "android", feature = "android-native-render")))]
pub fn render_char_android(
    _c: char,
    _size: f32,
    _color: [u8; 4],
) -> Option<crate::rasterizer::CachedChar> {
    None
}

