fn main() {
    // Build script for Android NDK integration
    println!("cargo:rerun-if-changed=build.rs");
    
    #[cfg(target_os = "android")]
    {
        println!("cargo:rustc-link-lib=log");
        println!("cargo:rustc-link-lib=android");
        // android-activity crate handles NativeActivity glue, but we need to ensure
        // the library is properly linked
    }
}

