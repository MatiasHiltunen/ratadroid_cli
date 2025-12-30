//! JNI utilities for safe and efficient Java interop
//!
//! This module provides thread-safe JNI operations with proper error handling
//! and thread attachment caching to avoid repeated attach/detach overhead.

use jni::objects::JObject;
use jni::sys::jobject;
use jni::{JavaVM, JNIEnv};
use log::{error, warn};
use ndk_context::android_context;
use std::sync::OnceLock;

/// Global JavaVM cache
static JAVA_VM: OnceLock<Option<JavaVM>> = OnceLock::new();

/// Initialize the JavaVM cache
/// Should be called early in the application lifecycle
pub fn init_java_vm() -> Result<(), String> {
    let ctx = android_context();
    let vm_ptr = ctx.vm();
    
    if vm_ptr.is_null() {
        return Err("JavaVM pointer is null".to_string());
    }
    
    unsafe {
        let vm = JavaVM::from_raw(vm_ptr as *mut _)
            .map_err(|e| format!("Failed to create JavaVM: {:?}", e))?;
        
        JAVA_VM.set(Some(vm)).map_err(|_| "JavaVM already initialized".to_string())?;
    }
    
    Ok(())
}

/// Get the cached JavaVM instance
pub fn get_java_vm() -> Result<&'static JavaVM, String> {
    // Try to get from cache first
    if let Some(vm) = JAVA_VM.get().and_then(|v| v.as_ref()) {
        return Ok(vm);
    }
    
    // If not cached, try to initialize
    init_java_vm()?;
    
    JAVA_VM
        .get()
        .and_then(|v| v.as_ref())
        .ok_or_else(|| "Failed to get JavaVM".to_string())
}

/// Execute a closure with a JNI environment
/// This handles thread attachment and ensures proper cleanup
/// 
/// Note: The closure must return owned values, not references to env
pub fn with_jni_env<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut JNIEnv) -> Result<R, String>,
{
    let vm = get_java_vm()?;
    let mut env = vm.attach_current_thread()
        .map_err(|e| format!("Failed to attach thread: {:?}", e))?;
    
    // SAFETY: We're only using env within this scope, and the closure
    // must return owned values, not references to env
    unsafe {
        let env_static: &mut JNIEnv<'static> = std::mem::transmute(&mut env);
        f(env_static)
    }
}

/// Get the Android context object
pub fn get_android_context() -> Result<jobject, String> {
    let ctx = android_context();
    let context_obj = ctx.context();
    
    if context_obj.is_null() {
        return Err("Android context is null".to_string());
    }
    
    // Cast from *mut c_void to jobject
    Ok(context_obj as jobject)
}

/// Call a Java method with proper error handling
/// Note: This is a simplified version that doesn't use the generic helper
/// due to lifetime complexities with JNI
pub fn call_void_method_simple(
    method_name: &str,
    signature: &str,
) -> Result<(), String> {
    with_jni_env(|env| {
        let context_obj = get_android_context()?;
        let context_jobj = unsafe { JObject::from_raw(context_obj) };
        
        let result = env.call_method(&context_jobj, method_name, signature, &[]);
        
        // Check for exceptions
        if handle_jni_exception(env, method_name) {
            return Err(format!("Exception in {}", method_name));
        }
        
        result.map_err(|e| format!("Failed to call {}: {:?}", method_name, e))?;
        Ok(())
    })
}

/// Handle JNI exceptions and log them appropriately
pub fn handle_jni_exception<'a>(env: &mut JNIEnv<'a>, context: &str) -> bool {
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

/// Call a Java method that returns an int array
pub fn call_int_array_method(
    method_name: &str,
    signature: &str,
) -> Result<Vec<i32>, String> {
    use jni::objects::JPrimitiveArray;
    use ndk_context::android_context;
    
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
        
        let context_jobj = JObject::from_raw(activity_obj as jni::sys::jobject);
        
        let result = env.call_method(&context_jobj, method_name, signature, &[]);
        
        if handle_jni_exception(&mut env, method_name) {
            return Err(format!("Exception calling {}", method_name));
        }
        
        let array_obj = result
            .map_err(|e| format!("Failed to call {}: {:?}", method_name, e))?
            .l()
            .map_err(|e| format!("Failed to convert to object: {:?}", e))?;
        
        let int_array: JPrimitiveArray<i32> = JPrimitiveArray::from_raw(array_obj.into_raw());
        let len = env
            .get_array_length(&int_array)
            .map_err(|e| format!("Failed to get array length: {:?}", e))? as usize;
        
        let mut values = vec![0i32; len];
        env.get_int_array_region(&int_array, 0, &mut values)
            .map_err(|e| format!("Failed to read array: {:?}", e))?;
        
        Ok(values)
    }
}

