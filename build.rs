//! Build script for ratadroid CLI
//!
//! This script sets up recompilation triggers for the template directory
//! so that changes to the template cause the CLI to be rebuilt.

use std::path::Path;
use walkdir::WalkDir;

/// Patterns for files/directories to exclude from template bundling
const EXCLUDE_PATTERNS: &[&str] = &[
    // Build artifacts
    "target",
    "build",
    ".gradle",
    // Generated/local files
    "local.properties",
    "Cargo.lock",
    // Native libraries (built from Rust)
    "jniLibs",
    // IDE files
    ".idea",
    "*.iml",
];

fn should_exclude(path: &Path) -> bool {
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        for pattern in EXCLUDE_PATTERNS {
            if pattern.starts_with('*') {
                // Glob pattern for extension
                let ext = pattern.trim_start_matches('*');
                if name.ends_with(ext) {
                    return true;
                }
            } else if name == *pattern {
                return true;
            }
        }
    }
    false
}

fn main() {
    let template_dir = Path::new("template");
    
    // Only set up rerun triggers if template directory exists
    if template_dir.exists() {
        // Trigger rebuild when template directory changes
        println!("cargo:rerun-if-changed=template");
        
        // Walk through template and set up individual file triggers
        // This allows more granular rebuilds
        for entry in WalkDir::new(template_dir)
            .into_iter()
            .filter_entry(|e| !should_exclude(e.path()))
        {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    println!("cargo:rerun-if-changed={}", entry.path().display());
                }
            }
        }
    }
    
    // Also trigger on old templates directory for backwards compatibility
    println!("cargo:rerun-if-changed=templates");
}

