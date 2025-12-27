//! Android native rendering for emojis and complex Unicode characters
//! Uses Android's Canvas/TextPaint APIs via JNI for proper color emoji support
//!
//! This module is gated behind the `android-native-backend` feature flag.
//! When enabled, it provides an alternative text rendering path using
//! Android's native Canvas/TextPaint APIs which can provide better
//! emoji support on some devices.

use jni::objects::{JObject, JPrimitiveArray};
use jni::sys::jobject;
use log;

use crate::rasterizer::{CachedChar, CHAR_CACHE};

/// Renders a character using Android's native TextPaint/Canvas APIs
/// Returns (width, height, is_wide, rgba_data) if successful
/// Uses LRU caching to avoid repeated JNI calls for the same character
#[cfg(target_os = "android")]
pub fn render_char_android(
    c: char,
    size: f32,
    color: [u8; 4],
) -> Option<CachedChar> {
    // Create cache key: (char_code, size_as_u32, color_as_u32)
    let size_u32 = size as u32;
    let color_u32 = ((color[3] as u32) << 24) |
                     ((color[0] as u32) << 16) |
                     ((color[1] as u32) << 8) |
                     (color[2] as u32);
    let cache_key = (c as u32, size_u32, color_u32);
    
    // Try to get from cache first (LRU .get() updates access time)
    {
        let mut cache = match CHAR_CACHE.lock() {
            Ok(cache) => cache,
            Err(e) => {
                log::warn!("Failed to lock character cache: {:?}", e);
                return None;
            }
        };
        if let Some(cached) = cache.get(&cache_key) {
            // Return cached copy (cloned to avoid holding lock)
            return Some((cached.0, cached.1, cached.2, cached.3.clone()));
        }
    }
    
    // Cache miss - render via JNI
    use ndk_context::android_context;
    
    let ctx = android_context();
    
    let vm_ptr = ctx.vm();
    let activity_obj = ctx.context();
    
    if vm_ptr.is_null() || activity_obj.is_null() {
        // Only log on first occurrence to avoid spam
        static LOGGED_NULL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED_NULL.swap(true, std::sync::atomic::Ordering::SeqCst) {
            log::warn!("VM or activity object is null");
        }
        return None;
    }
    
    // Use attach_current_thread (not permanently) to avoid threading issues
    unsafe {
        let vm = match jni::JavaVM::from_raw(vm_ptr as *mut _) {
            Ok(vm) => vm,
            Err(e) => {
                log::warn!("Failed to create JavaVM: {:?}", e);
                return None;
            }
        };
        
        // Attach thread temporarily - this is safer for multi-threaded rendering
        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                log::warn!("Failed to attach to Java VM: {:?}", e);
                return None;
            }
        };
        
        let activity_jobj = JObject::from_raw(activity_obj as jobject);
        
        // Call renderCharacter method on NativeActivity
        let char_str = c.to_string();
        let jstr = match env.new_string(&char_str) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to create JString: {:?}", e);
                return None;
            }
        };
        
        // Build color as ARGB integer
        let color_int = color_u32 as i32;
        
        // Call: byte[] renderCharacter(String character, float size, int color)
        // Signature: (Ljava/lang/String;FI)[B
        // Returns: [width(4), height(4), isWide(1), ...rgba_pixels] as byte array
        let result = match env.call_method(
            activity_jobj,
            "renderCharacter",
            "(Ljava/lang/String;FI)[B",
            &[
                jni::objects::JValue::Object(&jstr.into()),
                jni::objects::JValue::Float(size),
                jni::objects::JValue::Int(color_int),
            ],
        ) {
            Ok(result) => {
                let byte_array_obj = match result.l() {
                    Ok(arr) => arr,
                    Err(e) => {
                        log::warn!("Failed to get byte array from result: {:?}", e);
                        return None;
                    }
                };
                
                // Convert JObject to JPrimitiveArray<i8> (byte array in Java is signed)
                let byte_array = JPrimitiveArray::from_raw(byte_array_obj.into_raw());
                
                // Extract width, height, isWide, and pixel data
                let len = match env.get_array_length(&byte_array) {
                    Ok(l) => l as usize,
                    Err(e) => {
                        log::warn!("Failed to get array length: {:?}", e);
                        return None;
                    }
                };
                
                // New format: 9 bytes header (4+4+1)
                if len < 9 {
                    log::warn!("Byte array too short: {} bytes", len);
                    return None;
                }
                
                // Read the byte array (Java byte[] is signed, but we treat as unsigned)
                let mut signed_bytes = vec![0i8; len];
                match env.get_byte_array_region(&byte_array, 0, &mut signed_bytes[..]) {
                    Ok(_) => {},
                    Err(e) => {
                        log::warn!("Failed to read byte array: {:?}", e);
                        return None;
                    }
                };
                
                // Convert signed bytes to unsigned
                let bytes: Vec<u8> = signed_bytes.iter().map(|&b| b as u8).collect();
                
                // First 4 bytes: width (little-endian)
                let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                // Next 4 bytes: height (little-endian)
                let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                // Next byte: isWide flag
                let is_wide = bytes[8] != 0;
                
                // Check if Java returned empty array (exception occurred)
                if len == 0 {
                    // Log first few failures to avoid spam
                    static EMPTY_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    let count = EMPTY_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count < 10 {
                        log::warn!("Java renderCharacter returned empty array for '{}' (U+{:04X}) - likely exception", c, c as u32);
                    }
                    return None;
                }
                
                if width == 0 || height == 0 || len < 9 + (width * height * 4) as usize {
                    // Log first few failures to avoid spam
                    static INVALID_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                    let count = INVALID_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count < 10 {
                        log::warn!("Invalid dimensions or data for '{}' (U+{:04X}): {}x{}, len={}", c, c as u32, width, height, len);
                    }
                    return None;
                }
                
                // Remaining bytes: RGBA pixel data (starts at offset 9)
                let pixels = bytes[9..].to_vec();
                
                Some((width, height, is_wide, pixels))
            }
            Err(e) => {
                log::warn!("Failed to call renderCharacter for '{}': {:?}", c, e);
                if env.exception_check().unwrap_or(false) {
                    if let Err(desc_err) = env.exception_describe() {
                        log::warn!("Also failed to describe exception: {:?}", desc_err);
                    }
                    env.exception_clear().ok();
                }
                return None;
            }
        };
        
        // Cache the result before returning (LRU handles eviction automatically)
        if let Some((width, height, is_wide, ref pixels)) = result {
            if let Ok(mut cache) = CHAR_CACHE.lock() {
                cache.put(cache_key, (width, height, is_wide, pixels.clone()));
            }
        }
        
        result
    }
}

