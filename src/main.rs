//! Command‑line interface for the Ratadroid framework.
//!
//! This tool scaffolds complete Android NativeActivity projects with Rust integration,
//! manages Gradle-based builds, and provides a streamlined development workflow.

use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tokio::io::AsyncReadExt;
use hyper::{Body, Method, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use walkdir::WalkDir;
use std::env;
use std::fs as stdfs;

/// Embedded template directory - bundled at compile time
/// This includes the complete, runnable template project
static TEMPLATE_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/template");

/// Patterns for files/directories to exclude when extracting template
const TEMPLATE_EXCLUDE_PATTERNS: &[&str] = &[
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
];

/// Ratadroid CLI top‑level arguments.
#[derive(Parser)]
#[command(name = "ratadroid", version, about = "Robust CLI for Ratadroid Android development with Gradle", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Subcommands supported by the CLI.
#[derive(Subcommand)]
enum Commands {
    /// Print instructions for setting up the Android build toolchain.
    Init,
    /// Scaffold a new Android NativeActivity project with Rust integration.
    New {
        /// Project name (also used as package name).
        name: String,
        /// Target directory. Defaults to current directory.
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Build the Android project using Gradle.
    Build {
        /// Build variant: debug or release.
        #[arg(long, default_value = "debug")]
        variant: String,
        /// Target architecture (optional, builds all by default).
        #[arg(long)]
        target: Option<String>,
    },
    /// Install the APK on a connected device or emulator.
    Install {
        /// Build variant: debug or release.
        #[arg(long, default_value = "debug")]
        variant: String,
    },
    /// Run the app on a connected device or emulator.
    Run {
        /// Build variant: debug or release.
        #[arg(long, default_value = "debug")]
        variant: String,
    },
    /// Show crash logs from the last app run.
    Logs {
        /// Package name (optional, auto-detected from current directory).
        #[arg(long)]
        package: Option<String>,
        /// Number of lines to show.
        #[arg(long, default_value_t = 100)]
        lines: usize,
    },
    /// Build the current project into an Android binary using cargo-ndk (legacy).
    BuildLegacy {
        /// Target triple (e.g. aarch64-linux-android, armv7-linux-androideabi).
        #[arg(long)]
        target: Option<String>,
    },
    /// Serve a directory of APKs and other files over HTTP.
    Serve {
        /// Port to listen on.  Defaults to 8000.
        #[arg(long, default_value_t = 8000)]
        port: u16,
        /// Directory to serve.  Defaults to "dist".
        #[arg(long, default_value = "dist")] 
        dir: PathBuf,
    },
    /// Inspect the development environment and optionally fix issues.
    Doctor {
        /// Attempt to install or configure missing components automatically.
        #[arg(long, default_value_t = false)]
        fix: bool,
    },
    /// List all available Android devices and emulators.
    Devices,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => handle_init().await,
        Commands::New { name, path } => handle_new(name, path).await,
        Commands::Build { variant, target } => handle_gradle_build(variant, target).await,
        Commands::Install { variant } => handle_gradle_install(variant).await,
        Commands::Run { variant } => handle_gradle_run(variant).await,
        Commands::BuildLegacy { target } => handle_build(target).await,
        Commands::Serve { port, dir } => handle_serve(port, dir).await,
        Commands::Doctor { fix } => handle_doctor(fix).await,
        Commands::Logs { package, lines } => handle_logs(package, lines).await,
        Commands::Devices => handle_devices().await,
    }
}

/// Checks if Gradle is available and returns its path or wrapper path.
fn find_gradle(project_dir: Option<&Path>) -> Option<String> {
    // First check for Gradle wrapper in project directory
    if let Some(dir) = project_dir {
        let wrapper = if cfg!(windows) {
            dir.join("gradlew.bat")
        } else {
            dir.join("gradlew")
        };
        if wrapper.exists() {
            return Some(wrapper.to_string_lossy().to_string());
        }
    }
    
    // Check for global Gradle installation
    match Command::new("gradle").arg("--version").output() {
        Ok(output) if output.status.success() => Some("gradle".to_string()),
        _ => None,
    }
}

/// Performs best‑effort setup of the Android build environment.
async fn handle_init() -> Result<(), Box<dyn std::error::Error>> {
    println!("\nRatadroid development setup\n===========================\n");
    
    // Check for Gradle
    println!("Step 1: Checking for Gradle...");
    match find_gradle(None) {
        Some(gradle_path) => {
            let version_output = Command::new(&gradle_path).arg("--version").output()?;
            if version_output.status.success() {
                let version_str = String::from_utf8_lossy(&version_output.stdout);
                println!("  ✓ Gradle found: {}", version_str.lines().next().unwrap_or(""));
            }
        }
        None => {
            println!("  ⚠️  Gradle not found. Install from https://gradle.org/install/");
            println!("     Or it will be initialized automatically when creating a new project.");
        }
    }
    
    println!("\nStep 2: Ensure the Android NDK is installed.");
    println!("  Open Android Studio, go to \"SDK Manager\" > \"SDK Tools\",");
    println!("  check \"Android NDK (Side by side)\". Set ANDROID_NDK_HOME accordingly.\n");

    // Attempt to install cargo‑ndk
    println!("Step 3: Installing cargo‑ndk…");
    match Command::new("cargo")
        .args(["install", "cargo-ndk", "--force"])
        .status()
    {
        Ok(status) if status.success() => println!("  ✓ cargo‑ndk installed successfully."),
        Ok(status) => println!("  ⚠️ cargo‑ndk install exited with status {}. You may need to install it manually.", status),
        Err(err) => println!("  ⚠️ Failed to run cargo install: {}. Do you have cargo installed?", err),
    }

    // Attempt to add common Android targets via rustup
    println!("\nStep 4: Adding Rust Android targets…");
    match Command::new("rustup")
        .args(["target", "add",
            "aarch64-linux-android",
            "armv7-linux-androideabi",
            "i686-linux-android",
            "x86_64-linux-android",
        ])
        .status()
    {
        Ok(status) if status.success() => println!("  ✓ Rust targets installed."),
        Ok(status) => println!("  ⚠️ rustup target add exited with status {}. You may need to run this manually.", status),
        Err(err) => println!("  ⚠️ Failed to run rustup: {}. Install rustup from https://rustup.rs", err),
    }

    println!("\nSetup complete!");
    println!("You can now run `ratadroid new <project-name>` to scaffold a new Android project.");
    Ok(())
}

/// Detects Android SDK location from environment variables or common paths.
fn detect_android_sdk() -> Option<String> {
    let os = env::consts::OS;
    let mut sdk_paths = Vec::new();
    
    // Check environment variables first
    if let Ok(path) = env::var("ANDROID_SDK_ROOT") {
        sdk_paths.push(path);
    }
    if let Ok(path) = env::var("ANDROID_HOME") {
        if !sdk_paths.contains(&path) {
            sdk_paths.push(path);
        }
    }
    
    // Check common default locations
    let home = dirs::home_dir();
    match os {
        "windows" => {
            if let Some(home) = &home {
                let local_app = home.join("AppData").join("Local").join("Android").join("Sdk");
                sdk_paths.push(local_app.to_string_lossy().to_string());
            }
        }
        "macos" => {
            if let Some(home) = &home {
                let default = home.join("Library").join("Android").join("sdk");
                sdk_paths.push(default.to_string_lossy().to_string());
            }
        }
        _ => {
            if let Some(home) = &home {
                let default = home.join("Android").join("Sdk");
                sdk_paths.push(default.to_string_lossy().to_string());
            }
        }
    }
    
    // Find first existing path
    sdk_paths.retain(|p| !p.is_empty());
    sdk_paths.sort();
    sdk_paths.dedup();
    
    for p in &sdk_paths {
        if Path::new(p).exists() {
            return Some(p.clone());
        }
    }
    
    None
}

/// Detects Android NDK location.
fn detect_android_ndk() -> Option<String> {
    // Check ANDROID_NDK_HOME first
    if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
        if Path::new(&ndk_home).exists() {
            return Some(ndk_home);
        }
    }
    
    // Check in SDK directory
    if let Some(sdk) = detect_android_sdk() {
        let ndk_bundle = Path::new(&sdk).join("ndk-bundle");
        if ndk_bundle.exists() {
            return Some(ndk_bundle.to_string_lossy().to_string());
        }
        
        // Check ndk/<version> directories
        let ndk_dir = Path::new(&sdk).join("ndk");
        if ndk_dir.exists() {
            if let Ok(entries) = stdfs::read_dir(&ndk_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        return Some(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    
    None
}

/// Finds adb executable from SDK path or PATH.
fn find_adb() -> Option<String> {
    // First check if adb is in PATH
    match Command::new("adb").arg("version").output() {
        Ok(output) if output.status.success() => return Some("adb".to_string()),
        _ => {}
    }
    
    // Try to find adb in SDK platform-tools
    if let Some(sdk) = detect_android_sdk() {
        let adb_path = if cfg!(windows) {
            Path::new(&sdk).join("platform-tools").join("adb.exe")
        } else {
            Path::new(&sdk).join("platform-tools").join("adb")
        };
        if adb_path.exists() {
            return Some(adb_path.to_string_lossy().to_string());
        }
    }
    
    None
}

/// Finds emulator executable from SDK path or PATH.
fn find_emulator() -> Option<String> {
    // First check if emulator is in PATH
    if Command::new("emulator").arg("-version").output().is_ok() {
        return Some("emulator".to_string());
    }
    
    // Try to find emulator in SDK emulator directory
    if let Some(sdk) = detect_android_sdk() {
        let emulator_path = if cfg!(windows) {
            Path::new(&sdk).join("emulator").join("emulator.exe")
        } else {
            Path::new(&sdk).join("emulator").join("emulator")
        };
        if emulator_path.exists() {
            return Some(emulator_path.to_string_lossy().to_string());
        }
    }
    
    None
}

/// Device information structure
#[derive(Debug, Clone)]
struct DeviceInfo {
    id: String,
    state: String,
    is_physical: bool,
    model: String,
    product: String,
}

/// Checks if any Android devices are connected and ready.
fn has_connected_devices(adb: &str) -> bool {
    match Command::new(adb).args(["devices"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Check for device lines (not "List of devices" header or empty)
            // Only count devices that are in "device" state (not "offline" or "unauthorized")
            stdout.lines()
                .skip(1) // Skip "List of devices attached" header
                .any(|line| {
                    let trimmed = line.trim();
                    !trimmed.is_empty() && trimmed.ends_with("device")
                })
        }
        Err(_) => false,
    }
}

/// Lists all connected devices with their details.
fn list_devices(adb: &str) -> Vec<DeviceInfo> {
    let mut devices = Vec::new();
    
    match Command::new(adb).args(["devices", "-l"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                
                // Parse line format: "device_id    device state model:model_name product:product_name"
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }
                
                let device_id = parts[0].to_string();
                let state = parts.get(1).unwrap_or(&"unknown").to_string();
                
                // Only include devices in "device" state (ready)
                if state != "device" {
                    continue;
                }
                
                // Extract model and product from the rest of the line
                let mut model = String::new();
                let mut product = String::new();
                let mut is_physical = true; // Default to physical, check below
                
                for part in parts.iter().skip(2) {
                    if part.starts_with("model:") {
                        model = part.strip_prefix("model:").unwrap_or("").to_string();
                    } else if part.starts_with("product:") {
                        product = part.strip_prefix("product:").unwrap_or("").to_string();
                    }
                }
                
                // Check if it's an emulator (emulator devices typically have "sdk" or "emulator" in model/product)
                // Also check device ID - emulators usually start with "emulator-"
                if device_id.starts_with("emulator-") 
                    || model.to_lowercase().contains("sdk")
                    || model.to_lowercase().contains("emulator")
                    || product.to_lowercase().contains("sdk")
                    || product.to_lowercase().contains("emulator") {
                    is_physical = false;
                }
                
                // If model/product are empty, try to get them via adb shell
                if model.is_empty() || product.is_empty() {
                    if let Ok(prop_output) = Command::new(adb)
                        .args(["shell", "-s", &device_id, "getprop", "ro.product.model"])
                        .output() {
                        let prop_stdout = String::from_utf8_lossy(&prop_output.stdout);
                        if !prop_stdout.trim().is_empty() && model.is_empty() {
                            model = prop_stdout.trim().to_string();
                        }
                    }
                    
                    if let Ok(prop_output) = Command::new(adb)
                        .args(["shell", "-s", &device_id, "getprop", "ro.product.name"])
                        .output() {
                        let prop_stdout = String::from_utf8_lossy(&prop_output.stdout);
                        if !prop_stdout.trim().is_empty() && product.is_empty() {
                            product = prop_stdout.trim().to_string();
                        }
                    }
                }
                
                devices.push(DeviceInfo {
                    id: device_id,
                    state,
                    is_physical,
                    model: if model.is_empty() { "Unknown".to_string() } else { model },
                    product: if product.is_empty() { "Unknown".to_string() } else { product },
                });
            }
        }
        Err(_) => {}
    }
    
    devices
}

/// Gets the first available physical device, or any device if no physical device is available.
fn get_preferred_device(adb: &str) -> Option<DeviceInfo> {
    let devices = list_devices(adb);
    
    // Prefer physical devices
    if let Some(physical) = devices.iter().find(|d| d.is_physical) {
        return Some(physical.clone());
    }
    
    // Fall back to any device
    devices.first().cloned()
}

/// Lists available Android Virtual Devices (AVDs).
fn list_avds(emulator: &str) -> Vec<String> {
    match Command::new(emulator).arg("-list-avds").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Starts an emulator with the given AVD name and waits for it to boot.
async fn start_emulator(emulator: &str, avd_name: &str, adb: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Check if emulator is already running (might be booting)
    if has_connected_devices(adb) {
        println!("✓ Device/emulator already available");
        return Ok(());
    }
    
    println!("Starting emulator: {}...", avd_name);
    
    // Start emulator in background (detached, so it continues running)
    let mut child = Command::new(emulator)
        .args(["-avd", avd_name])
        .spawn()
        .map_err(|e| format!("Failed to start emulator: {}", e))?;
    
    // Wait a moment for emulator to initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    println!("Waiting for emulator to boot...");
    
    // Wait for device to be detected by adb
    let wait_status = Command::new(adb)
        .args(["wait-for-device"])
        .status();
    
    if wait_status.is_err() || !wait_status.unwrap().success() {
        // Try to kill the emulator process if wait failed
        let _ = child.kill();
        return Err("Failed to wait for emulator device".into());
    }
    
    // Wait for boot completion (check bootanim property)
    println!("Waiting for boot completion...");
    let mut booted = false;
    for i in 0..120 { // Wait up to 2 minutes (120 * 1 second)
        // Show progress every 10 seconds
        if i > 0 && i % 10 == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
        
        let boot_status = Command::new(adb)
            .args(["shell", "getprop", "sys.boot_completed"])
            .output();
        
        if let Ok(output) = boot_status {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim() == "1" {
                booted = true;
                if i > 0 {
                    println!(); // New line after progress dots
                }
                break;
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    
    if !booted {
        println!(); // New line if we didn't break early
        return Err("Emulator did not boot within timeout period (2 minutes)".into());
    }
    
    println!("✓ Emulator is ready!");
    // Note: We don't wait for the child process - emulator runs in background
    // This is intentional - the emulator should keep running
    Ok(())
}

/// Ensures a device or emulator is available, starting one if needed.
/// Prefers physical devices over emulators.
/// Returns the device ID of the selected device, or None if multiple devices are available and we should let adb choose.
async fn ensure_device_available() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let adb = find_adb()
        .ok_or("adb not found. Make sure Android SDK platform-tools is installed.")?;
    
    // Check if devices are already connected
    if has_connected_devices(&adb) {
        // Check if we have a physical device (preferred)
        let devices = list_devices(&adb);
        let has_physical = devices.iter().any(|d| d.is_physical);
        
        if has_physical {
            let physical_devices: Vec<_> = devices.iter().filter(|d| d.is_physical).collect();
            if physical_devices.len() == 1 {
                println!("✓ Using physical device: {} ({})", 
                    physical_devices[0].model, physical_devices[0].id);
                return Ok(Some(physical_devices[0].id.clone()));
            } else {
                // Multiple physical devices - prefer the first one
                println!("✓ {} physical device(s) available, using: {} ({})", 
                    physical_devices.len(),
                    physical_devices[0].model, physical_devices[0].id);
                return Ok(Some(physical_devices[0].id.clone()));
            }
        } else {
            // Only emulators available
            if devices.len() == 1 {
                println!("✓ Using emulator: {}", devices[0].id);
                return Ok(Some(devices[0].id.clone()));
            } else {
                // Multiple emulators - use the first one
                println!("✓ {} emulator(s) available, using: {}", devices.len(), devices[0].id);
                return Ok(Some(devices[0].id.clone()));
            }
        }
    }
    
    // No devices connected, try to start an emulator
    let emulator = find_emulator()
        .ok_or("No devices connected and emulator not found. Please connect a device or install Android emulator.")?;
    
    let avds = list_avds(&emulator);
    if avds.is_empty() {
        return Err("No devices connected and no AVDs available. Please create an AVD using Android Studio or connect a device.".into());
    }
    
    // Use the first available AVD
    let avd_name = &avds[0];
    if avds.len() > 1 {
        println!("Multiple AVDs available, using: {}", avd_name);
    }
    
    start_emulator(&emulator, avd_name, &adb).await?;
    
    // After starting emulator, get the device ID
    let devices = list_devices(&adb);
    if let Some(device) = devices.first() {
        Ok(Some(device.id.clone()))
    } else {
        Ok(None)
    }
}

/// Lists all available devices and emulators.
async fn handle_devices() -> Result<(), Box<dyn std::error::Error>> {
    let adb = find_adb()
        .ok_or("adb not found. Make sure Android SDK platform-tools is installed.")?;
    
    println!("{}", "═".repeat(80));
    println!("AVAILABLE DEVICES");
    println!("{}", "═".repeat(80));
    println!();
    
    let devices = list_devices(&adb);
    
    if devices.is_empty() {
        println!("No devices connected.");
        println!();
        
        // Check for available AVDs
        if let Some(emulator) = find_emulator() {
            let avds = list_avds(&emulator);
            if !avds.is_empty() {
                println!("Available AVDs (not running):");
                for (i, avd) in avds.iter().enumerate() {
                    println!("  {}. {}", i + 1, avd);
                }
                println!();
                println!("Start an emulator with: ratadroid run");
            } else {
                println!("No AVDs available. Create one using Android Studio.");
            }
        } else {
            println!("Emulator not found. Install Android emulator via Android Studio.");
        }
        
        return Ok(());
    }
    
    // Separate physical devices and emulators
    let physical_devices: Vec<_> = devices.iter().filter(|d| d.is_physical).collect();
    let emulators: Vec<_> = devices.iter().filter(|d| !d.is_physical).collect();
    
    if !physical_devices.is_empty() {
        println!("📱 PHYSICAL DEVICES:");
        println!("{}", "─".repeat(80));
        for device in &physical_devices {
            println!("  ID:       {}", device.id);
            println!("  Model:    {}", device.model);
            println!("  Product:  {}", device.product);
            println!("  State:    {} {}", device.state, if device.state == "device" { "✓" } else { "" });
            println!();
        }
    }
    
    if !emulators.is_empty() {
        println!("🖥️  EMULATORS:");
        println!("{}", "─".repeat(80));
        for device in &emulators {
            println!("  ID:       {}", device.id);
            println!("  Model:    {}", device.model);
            println!("  Product:  {}", device.product);
            println!("  State:    {} {}", device.state, if device.state == "device" { "✓" } else { "" });
            println!();
        }
    }
    
    // Show available but not running AVDs
    if let Some(emulator) = find_emulator() {
        let avds = list_avds(&emulator);
        let running_avd_names: Vec<String> = emulators.iter()
            .map(|d| {
                // Try to extract AVD name from emulator device
                // This is approximate - emulator IDs don't directly map to AVD names
                d.id.clone()
            })
            .collect();
        
        let not_running: Vec<_> = avds.iter()
            .filter(|avd| {
                // Check if this AVD is already running
                // This is approximate - we can't perfectly match AVD names to device IDs
                !running_avd_names.iter().any(|id| id.contains(avd.as_str()))
            })
            .collect();
        
        if !not_running.is_empty() {
            println!("💤 AVAILABLE AVDs (not running):");
            println!("{}", "─".repeat(80));
            for avd in &not_running {
                println!("  • {}", avd);
            }
            println!();
        }
    }
    
    println!("{}", "═".repeat(80));
    if !physical_devices.is_empty() {
        println!("Note: Physical devices are preferred when running apps.");
    }
    
    Ok(())
}

/// Checks if a path component should be excluded when extracting template
fn should_exclude_template_path(path: &Path) -> bool {
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        for pattern in TEMPLATE_EXCLUDE_PATTERNS {
            if name == *pattern {
                return true;
            }
        }
    }
    false
}

/// Applies placeholder replacement to template content
/// Replaces template placeholders with project-specific values
fn apply_template_replacements(content: &str, project_name: &str) -> String {
    content
        // Package name: template -> project_name
        .replace("com.ratadroid.template", &format!("com.ratadroid.{}", project_name))
        // Project name in settings.gradle
        .replace("rootProject.name = 'ratadroid_template'", &format!("rootProject.name = '{}'", project_name))
        .replace("rootProject.name = \"ratadroid_template\"", &format!("rootProject.name = \"{}\"", project_name))
        // General template references
        .replace("ratadroid_template", project_name)
}

/// Files that need template placeholder replacement
fn needs_template_replacement(path: &Path) -> bool {
    let replaceable_extensions = ["gradle", "xml", "java", "kt", "rs", "toml", "md", "properties"];
    let replaceable_names = ["gradlew", "gradlew.bat"];
    
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if replaceable_names.contains(&name) {
            return false; // Scripts don't need replacement
        }
    }
    
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        return replaceable_extensions.contains(&ext);
    }
    false
}

/// Extracts the bundled template to a target directory with project-specific modifications
async fn extract_template(
    project_dir: &Path,
    project_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use include_dir::DirEntry;
    
    // Helper to recursively extract files
    fn collect_entries<'a>(dir: &'a Dir<'a>, entries: &mut Vec<(&'a Path, &'a [u8])>) {
        for entry in dir.entries() {
            match entry {
                DirEntry::Dir(subdir) => {
                    collect_entries(subdir, entries);
                }
                DirEntry::File(file) => {
                    entries.push((file.path(), file.contents()));
                }
            }
        }
    }
    
    let mut entries = Vec::new();
    collect_entries(&TEMPLATE_DIR, &mut entries);
    
    for (rel_path, contents) in entries {
        // Skip excluded paths
        if should_exclude_template_path(rel_path) {
            continue;
        }
        
        // Transform the path for the new project
        let mut target_path = project_dir.to_path_buf();
        
        // Handle Java package directory renaming
        // template/app/src/main/java/com/ratadroid/template/... -> .../com/ratadroid/{name}/...
        let path_str = rel_path.to_string_lossy();
        if path_str.contains("com/ratadroid/template") {
            let new_path_str = path_str.replace(
                "com/ratadroid/template",
                &format!("com/ratadroid/{}", project_name),
            );
            target_path.push(&new_path_str);
        } else {
            target_path.push(rel_path);
        }
        
        // Create parent directories
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Apply template replacements for text files
        if needs_template_replacement(rel_path) {
            if let Ok(text) = std::str::from_utf8(contents) {
                let modified = apply_template_replacements(text, project_name);
                fs::write(&target_path, modified).await?;
            } else {
                // Binary file, write as-is
                fs::write(&target_path, contents).await?;
            }
        } else {
            // Binary file or no replacement needed
            fs::write(&target_path, contents).await?;
        }
    }
    
    // Make gradlew executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let gradlew = project_dir.join("gradlew");
        if gradlew.exists() {
            let mut perms = stdfs::metadata(&gradlew)?.permissions();
            perms.set_mode(0o755);
            stdfs::set_permissions(&gradlew, perms)?;
        }
    }
    
    Ok(())
}

/// Scaffolds a new Android NativeActivity project with Rust integration.
/// 
/// Uses the bundled template directory which is a complete, runnable project.
/// The template is extracted and customized with the project name.
async fn handle_new(name: String, path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = path.unwrap_or_else(|| PathBuf::from(&name));
    
    if project_dir.exists() {
        return Err(format!("Directory {} already exists", project_dir.display()).into());
    }
    
    println!("Scaffolding Android NativeActivity project: {}", name);
    println!("Project directory: {}", project_dir.display());
    
    // Detect Android SDK and NDK
    let sdk_path = detect_android_sdk();
    let ndk_path = detect_android_ndk();
    
    if let Some(sdk) = &sdk_path {
        println!("Detected Android SDK at: {}", sdk);
    }
    if let Some(ndk) = &ndk_path {
        println!("Detected Android NDK at: {}", ndk);
    }
    
    // Create project directory
    fs::create_dir_all(&project_dir).await?;
    
    // Extract the bundled template
    println!("Extracting template...");
    extract_template(&project_dir, &name).await?;
    
    // Create local.properties with SDK location if found
    if let Some(sdk) = &sdk_path {
        // Escape backslashes for Windows paths in properties file
        let sdk_path_escaped = sdk.replace('\\', "\\\\");
        let local_properties = format!("sdk.dir={}\n", sdk_path_escaped);
        fs::write(project_dir.join("local.properties"), local_properties).await?;
        println!("Created local.properties with SDK location");
    } else {
        println!("⚠️  Android SDK not detected. You may need to create local.properties manually.");
    }
    
    // Verify Gradle wrapper is available
    let gradle = find_gradle(Some(&project_dir));
    if let Some(g) = &gradle {
        println!("Using Gradle: {}", g);
    }
    
    println!("\n✓ Project scaffolded successfully!");
    
    // Set environment variables for current process if not already set
    if let Some(sdk) = &sdk_path {
        if env::var("ANDROID_SDK_ROOT").is_err() && env::var("ANDROID_HOME").is_err() {
            println!("\n💡 Tip: Set ANDROID_SDK_ROOT or ANDROID_HOME environment variable:");
            println!("   export ANDROID_SDK_ROOT={}", sdk);
        }
    }
    if let Some(ndk) = &ndk_path {
        if env::var("ANDROID_NDK_HOME").is_err() {
            println!("\n💡 Tip: Set ANDROID_NDK_HOME environment variable:");
            println!("   export ANDROID_NDK_HOME={}", ndk);
        }
    }
    
    println!("\nNext steps:");
    println!("  1. cd {}", project_dir.display());
    if sdk_path.is_none() {
        println!("  2. Create local.properties with: sdk.dir=<path-to-android-sdk>");
        println!("  3. ratadroid doctor --fix  # Verify environment");
    } else {
        println!("  2. ratadroid build        # Build the APK");
        println!("  3. ratadroid run          # Build, install, and run on device");
    }
    
    Ok(())
}

/// Builds the Android project using Gradle.
async fn handle_gradle_build(variant: String, _target: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = env::current_dir()?;
    let gradle = find_gradle(Some(&project_dir))
        .ok_or("Gradle not found. Run 'ratadroid init' or ensure Gradle is installed.")?;
    
    println!("Building Android project (variant: {})...", variant);
    
    let task = format!("assemble{}", capitalize_first(&variant));
    let status = Command::new(&gradle)
        .current_dir(&project_dir)
        .arg(&task)
        .status()?;
    
    if status.success() {
        println!("\n✓ Build succeeded!");
        let apk_path = project_dir.join("app").join("build").join("outputs").join("apk")
            .join(&variant).join(format!("app-{}.apk", variant));
        if apk_path.exists() {
            println!("APK location: {}", apk_path.display());
        }
    } else {
        return Err(format!("Build failed with exit status {}", status.code().unwrap_or(-1)).into());
    }
    
    Ok(())
}

/// Installs the APK on a connected device or emulator.
async fn handle_gradle_install(variant: String) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = env::current_dir()?;
    let gradle = find_gradle(Some(&project_dir))
        .ok_or("Gradle not found. Run 'ratadroid init' or ensure Gradle is installed.")?;
    
    // Ensure a device is available before attempting installation
    // Get the device ID to pass to Gradle if multiple devices are connected
    let device_id = ensure_device_available().await?;
    
    println!("Installing APK (variant: {})...", variant);
    
    // For release builds, installRelease task typically doesn't exist unless the build is signed
    // So we'll use adb install directly for release builds
    // For debug builds, try Gradle install task first
    let use_gradle_install = variant == "debug";
    
    if use_gradle_install {
        let task = format!("install{}", capitalize_first(&variant));
        let mut gradle_cmd = Command::new(&gradle);
        gradle_cmd.current_dir(&project_dir).arg(&task);
        
        // If we have a specific device ID and multiple devices are connected, tell Gradle which one to use
        if let Some(device_id) = &device_id {
            // Set ANDROID_SERIAL environment variable to tell Gradle/adb which device to use
            gradle_cmd.env("ANDROID_SERIAL", device_id);
        }
        
        let status = gradle_cmd.status()?;
        
        if status.success() {
            println!("\n✓ Installation succeeded!");
        } else {
            return Err(format!("Installation failed with exit status {}", status.code().unwrap_or(-1)).into());
        }
    } else {
        // Fallback to adb install for release builds or when install task doesn't exist
        let adb = find_adb()
            .ok_or("adb not found. Make sure Android SDK platform-tools is installed.")?;
        
        // Find the APK file
        // Release builds are typically named app-release-unsigned.apk
        let apk_dir = project_dir
            .join("app")
            .join("build")
            .join("outputs")
            .join("apk")
            .join(&variant);
        
        // Try signed release APK first, then unsigned, then standard naming
        let apk_path = if variant == "release" {
            // Prefer signed release APK (if signingConfig is configured)
            let signed_path = apk_dir.join("app-release.apk");
            let unsigned_path = apk_dir.join("app-release-unsigned.apk");
            
            if signed_path.exists() {
                signed_path
            } else if unsigned_path.exists() {
                unsigned_path
            } else {
                return Err(format!("Release APK not found. Expected at {} or {}", 
                    signed_path.display(), unsigned_path.display()).into());
            }
        } else {
            apk_dir.join(format!("app-{}.apk", variant))
        };
        
        if !apk_path.exists() {
            return Err(format!("APK not found at {}. Build the project first with 'ratadroid build --variant {}'", 
                apk_path.display(), variant).into());
        }
        
        println!("Using adb install (install{} task not available)...", capitalize_first(&variant));
        
        let mut adb_cmd = Command::new(&adb);
        
        // If we have a specific device ID, use it
        if let Some(device_id) = &device_id {
            adb_cmd.args(["-s", device_id]);
        }
        
        // Install APK (signed release APKs don't need -t flag)
        adb_cmd.args(["install", "-r", apk_path.to_str().unwrap()]);
        
        let output = adb_cmd.output()?;
        
        if output.status.success() {
            println!("\n✓ Installation succeeded!");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Provide helpful error message for unsigned release APKs
            if stderr.contains("INSTALL_PARSE_FAILED_NO_CERTIFICATES") || 
               stderr.contains("no certificates") {
                return Err(format!(
                    "Installation failed: Release APK is unsigned.\n  \
                    The Gradle build should automatically sign release builds with debug keystore.\n  \
                    If this error persists, try:\n  \
                    1. Use debug builds: ratadroid run --variant debug\n  \
                    2. Check that signingConfig is configured in app/build.gradle"
                ).into());
            }
            
            let error_msg = if !stderr.is_empty() {
                stderr.to_string()
            } else if !stdout.is_empty() {
                stdout.to_string()
            } else {
                format!("Installation failed with exit status {}", output.status.code().unwrap_or(-1))
            };
            
            return Err(format!("Installation failed: {}", error_msg.trim()).into());
        }
    }
    
    Ok(())
}

/// Runs the app on a connected device or emulator.
async fn handle_gradle_run(variant: String) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = env::current_dir()?;
    println!("Running app (variant: {})...", variant);
    
    // First build and install
    handle_gradle_build(variant.clone(), None).await?;
    handle_gradle_install(variant.clone()).await?;
    
    // Find adb automatically
    let adb = find_adb()
        .ok_or("adb not found. Make sure Android SDK platform-tools is installed and accessible.")?;
    
    // Get the preferred device ID (physical device preferred)
    let device_id = get_preferred_device(&adb);
    
    // Then launch
    let package_name = format!("com.ratadroid.{}", 
        project_dir.file_name().and_then(|n| n.to_str()).unwrap_or("app"));
    
    println!("Launching app...");
    let mut launch_cmd = Command::new(&adb);
    
    // If we have a device ID and multiple devices are connected, specify which device to use
    if let Some(device_info) = &device_id {
        launch_cmd.args(["-s", &device_info.id]);
        println!("  Targeting device: {} ({})", device_info.model, device_info.id);
    }
    
    launch_cmd.args(["shell", "am", "start", "-n", &format!("{}/.NativeActivity", package_name)]);
    
    let status = launch_cmd.status();
    
    match status {
        Ok(s) if s.success() => println!("✓ App launched!"),
        Ok(_) => {
            let cmd_str = if let Some(device_info) = &device_id {
                format!("{} -s {} shell am start -n {}/.NativeActivity", adb, device_info.id, package_name)
            } else {
                format!("{} shell am start -n {}/.NativeActivity", adb, package_name)
            };
            println!("⚠️  Launch command failed. Try manually: {}", cmd_str);
        },
        Err(e) => return Err(format!("Failed to launch app: {}", e).into()),
    }
    
    Ok(())
}

/// Shows crash logs and error messages from the app.
async fn handle_logs(package: Option<String>, lines: usize) -> Result<(), Box<dyn std::error::Error>> {
    let package_name = if let Some(pkg) = package {
        pkg
    } else {
        // Try to detect from current directory
        let project_dir = env::current_dir()?;
        let dir_name = project_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app");
        format!("com.ratadroid.{}", dir_name)
    };
    
    let adb = find_adb()
        .ok_or("adb not found. Make sure Android SDK platform-tools is installed and accessible.")?;
    
    println!("Fetching logs for package: {}\n", package_name);
    println!("{}", "═".repeat(80));
    println!("CRASH LOGS & ERRORS");
    println!("{}", "═".repeat(80));
    println!();
    
    // Get AndroidRuntime crashes
    let runtime_output = Command::new(&adb)
        .args(["logcat", "-d", "-s", "AndroidRuntime:E"])
        .output()?;
    
    if runtime_output.status.success() {
        let runtime_logs = String::from_utf8_lossy(&runtime_output.stdout);
        if !runtime_logs.trim().is_empty() {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("ANDROID RUNTIME CRASHES:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            for line in runtime_logs.lines().take(lines) {
                if line.contains(&package_name) || line.contains("FATAL") || line.contains("Exception") {
                    println!("{}", line);
                }
            }
            println!();
        }
    }
    
    // Get app-specific logs
    let app_output = Command::new(&adb)
        .args(["logcat", "-d", "-s", &format!("{}:*", package_name)])
        .output()?;
    
    if app_output.status.success() {
        let app_logs = String::from_utf8_lossy(&app_output.stdout);
        if !app_logs.trim().is_empty() {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("APP LOGS:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            for line in app_logs.lines().take(lines) {
                println!("{}", line);
            }
            println!();
        }
    }
    
    // Get general errors
    let error_output = Command::new(&adb)
        .args(["logcat", "-d", "-s", "*:E"])
        .output()?;
    
    if error_output.status.success() {
        let error_logs = String::from_utf8_lossy(&error_output.stdout);
        let relevant_errors: Vec<&str> = error_logs
            .lines()
            .filter(|line| line.contains(&package_name))
            .take(lines)
            .collect();
        
        if !relevant_errors.is_empty() {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("ERROR LOGS:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            for line in relevant_errors {
                println!("{}", line);
            }
        }
    }
    
    println!();
    println!("{}", "═".repeat(80));
    println!("Tip: Use 'adb logcat -c' to clear logs, or 'ratadroid logs --lines 200' for more");
    println!("{}", "═".repeat(80));
    
    Ok(())
}

/// Capitalizes the first letter of a string.
fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Runs cargo‑ndk to build the current crate for the given target architecture (legacy).
async fn handle_build(target: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let target = target.unwrap_or_else(|| "aarch64-linux-android".to_string());
    println!("Building for target {} (legacy cargo-ndk method)…", target);
    let status = Command::new("cargo")
        .args(["ndk", "--target", &target, "-o", "dist", "build", "--release"])
        .status();
    match status {
        Ok(exit) if exit.success() => {
            println!("\nBuild succeeded.  The output should be in the `dist` directory.");
        }
        Ok(exit) => {
            println!("\nBuild failed with status {}.  Ensure cargo‑ndk is installed.", exit);
        }
        Err(err) => {
            println!("\nError launching cargo‑ndk: {}.  Is cargo‑ndk installed?", err);
        }
    }
    Ok(())
}

/// Checks the developer's environment and prints a report.
async fn handle_doctor(fix: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nRatadroid doctor\n==============\n");
    let os = env::consts::OS;
    println!("Operating system: {}", os);

    // Check Gradle
    println!("\nChecking Gradle...");
    match find_gradle(None) {
        Some(gradle) => {
            let version_output = Command::new(&gradle).arg("--version").output();
            match version_output {
                Ok(out) if out.status.success() => {
                    let version_str = String::from_utf8_lossy(&out.stdout);
                    println!("  ✓ Gradle found: {}", version_str.lines().next().unwrap_or(""));
                }
                _ => println!("  ⚠️  Gradle found but version check failed."),
            }
        }
        None => {
            println!("  ⚠️  Gradle not found. Install from https://gradle.org/install/");
            if fix {
                println!("  Note: Gradle wrapper will be initialized when creating a new project.");
            }
        }
    }

    // Determine Android SDK path using helper function
    let detected_sdk = detect_android_sdk();
    
    println!("\nChecking Android SDK...");
    if let Some(sdk) = &detected_sdk {
        println!("  ✓ Found Android SDK at: {}", sdk);
    } else {
        println!("  ⚠️  Android SDK not found. Set ANDROID_SDK_ROOT or ANDROID_HOME.");
    }

    // Check cargo-ndk
    println!("\nChecking cargo-ndk...");
    let cargo_ndk_installed = Command::new("cargo")
        .args(["ndk", "--version"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false);
    if cargo_ndk_installed {
        println!("  ✓ cargo-ndk is installed.");
    } else {
        println!("  ⚠️  cargo-ndk not found. Install with `cargo install cargo-ndk`.");
        if fix {
            println!("  Attempting to install cargo‑ndk…");
            let _ = Command::new("cargo").args(["install", "cargo-ndk", "--force"]).status();
        }
    }

    // Check Rust Android targets
    println!("\nChecking Rust Android targets...");
    let required_targets = vec![
        "aarch64-linux-android",
        "armv7-linux-androideabi",
        "i686-linux-android",
        "x86_64-linux-android",
    ];
    let installed_targets_output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();
    let mut missing_targets = Vec::new();
    match installed_targets_output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for t in &required_targets {
                if !stdout.contains(*t) {
                    missing_targets.push(*t);
                }
            }
            if missing_targets.is_empty() {
                println!("  ✓ All Rust Android targets are installed.");
            } else {
                println!("  ⚠️  Missing Rust targets: {:?}", missing_targets);
                if fix {
                    println!("  Adding missing Rust targets…");
                    let mut args = vec!["target", "add"];
                    for t in &missing_targets {
                        args.push(t);
                    }
                    let _ = Command::new("rustup").args(&args).status();
                }
            }
        }
        _ => println!("  ⚠️  Could not determine installed Rust targets. Ensure rustup is installed."),
    }

    // Check NDK using helper function
    println!("\nChecking Android NDK...");
    let detected_ndk = detect_android_ndk();
    if let Some(ndk) = &detected_ndk {
        println!("  ✓ Found NDK at: {}", ndk);
    } else {
        println!("  ⚠️  NDK not found. Install via SDK Manager or set ANDROID_NDK_HOME.");
    }

    println!("\nDoctor check complete. Review the messages above.");
    Ok(())
}

/// Serves files from the given directory on the specified port using Hyper.
async fn handle_serve(port: u16, dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let dir = fs::canonicalize(&dir).await?;
    println!("Serving {} on http://0.0.0.0:{}/", dir.display(), port);
    let make_service = make_service_fn(move |_| {
        let dir = dir.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                let dir = dir.clone();
                async move {
                    match (req.method(), req.uri().path()) {
                        (&Method::GET, "/") => {
                            let mut listing = String::from("<html><body><h1>Files</h1><ul>");
                            for entry in WalkDir::new(&dir).min_depth(1).max_depth(1) {
                                let entry = match entry {
                                    Ok(e) => e,
                                    Err(_) => continue,
                                };
                                if entry.path().is_file() {
                                    let name = entry.file_name().to_string_lossy();
                                    listing.push_str(&format!("<li><a href=\"/{0}\">{0}</a></li>", name));
                                }
                            }
                            listing.push_str("</ul></body></html>");
                            Ok::<_, hyper::Error>(Response::new(Body::from(listing)))
                        }
                        (&Method::GET, path) => {
                            let trimmed = path.trim_start_matches('/');
                            if trimmed.contains("..") || trimmed.starts_with('/') {
                                return Ok(Response::builder()
                                    .status(403)
                                    .body(Body::from("Forbidden: Invalid path"))
                                    .unwrap());
                            }
                            let mut file_path = dir.clone();
                            file_path.push(trimmed);
                            match fs::canonicalize(&file_path).await {
                                Ok(canonical) => {
                                    if !canonical.starts_with(&dir) {
                                        return Ok(Response::builder()
                                            .status(403)
                                            .body(Body::from("Forbidden: Path traversal detected"))
                                            .unwrap());
                                    }
                                    match fs::File::open(&canonical).await {
                                        Ok(mut file) => {
                                            let mut data = Vec::new();
                                            match file.read_to_end(&mut data).await {
                                                Ok(_) => {
                                                    let mime = if canonical.extension().and_then(|s| s.to_str()) == Some("apk") {
                                                        "application/vnd.android.package-archive"
                                                    } else {
                                                        "application/octet-stream"
                                                    };
                                                    let response = Response::builder()
                                                        .header("Content-Type", mime)
                                                        .body(Body::from(data))
                                                        .unwrap();
                                                    Ok(response)
                                                }
                                                Err(_e) => {
                                                    Ok(Response::builder()
                                                        .status(500)
                                                        .body(Body::from("Internal Server Error"))
                                                        .unwrap())
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            Ok(Response::builder()
                                                .status(404)
                                                .body(Body::from("Not Found"))
                                                .unwrap())
                                        }
                                    }
                                }
                                Err(_) => {
                                    Ok(Response::builder()
                                        .status(404)
                                        .body(Body::from("Not Found"))
                                        .unwrap())
                                }
                            }
                        }
                        _ => Ok(Response::builder()
                            .status(405)
                            .body(Body::from("Method Not Allowed"))
                            .unwrap()),
                    }
                }
            }))
        }
    });
    let addr = ([0, 0, 0, 0], port).into();
    Server::bind(&addr).serve(make_service).await?;
    Ok(())
}
