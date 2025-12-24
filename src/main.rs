//! Command‑line interface for the Ratadroid framework.
//!
//! This tool scaffolds complete Android NativeActivity projects with Rust integration,
//! manages Gradle-based builds, and provides a streamlined development workflow.

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tokio::io::AsyncReadExt;
use hyper::{Body, Method, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use walkdir::WalkDir;
use std::env;
use std::fs as stdfs;

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

/// Ensures Gradle is available, installing wrapper if needed.
async fn ensure_gradle(project_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    // Check if wrapper exists
    let wrapper = if cfg!(windows) {
        project_dir.join("gradlew.bat")
    } else {
        project_dir.join("gradlew")
    };
    
    if wrapper.exists() {
        return Ok(wrapper.to_string_lossy().to_string());
    }
    
    // Check for global Gradle
    if let Some(gradle) = find_gradle(None) {
        // Initialize Gradle wrapper
        println!("Initializing Gradle wrapper...");
        let status = Command::new(&gradle)
            .current_dir(project_dir)
            .args(["wrapper", "--gradle-version", "8.5"])
            .status()?;
        
        if status.success() {
            let wrapper_path = if cfg!(windows) {
                project_dir.join("gradlew.bat")
            } else {
                project_dir.join("gradlew")
            };
            // Make wrapper executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = stdfs::metadata(&wrapper_path)?.permissions();
                perms.set_mode(0o755);
                stdfs::set_permissions(&wrapper_path, perms)?;
            }
            return Ok(wrapper_path.to_string_lossy().to_string());
        }
    }
    
    Err("Gradle not found. Please install Gradle or run 'ratadroid init' first.".into())
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
#[allow(dead_code)]
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

/// Scaffolds a new Android NativeActivity project with Rust integration.
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
    
    // Create directory structure
    fs::create_dir_all(&project_dir).await?;
    
    let app_dir = project_dir.join("app");
    let src_main_dir = app_dir.join("src").join("main");
    let java_dir = src_main_dir.join("java").join("com").join("ratadroid").join(&name);
    let res_dir = src_main_dir.join("res");
    let rust_dir = project_dir.join("rust");
    
    fs::create_dir_all(&java_dir).await?;
    fs::create_dir_all(&res_dir.join("layout")).await?;
    fs::create_dir_all(&res_dir.join("values")).await?;
    fs::create_dir_all(&rust_dir.join("src")).await?;
    
    // Helper function to replace placeholders in templates
    let replace_template = |template: &str| -> String {
        template.replace("{name}", &name)
    };
    
    // Create root build.gradle FIRST (before initializing wrapper)
    let root_build_gradle = include_str!("../templates/root_build.gradle");
    fs::write(project_dir.join("build.gradle"), root_build_gradle).await?;
    
    // Create settings.gradle
    let settings_gradle = format!(r#"rootProject.name = "{}"
include ':app'
"#, name);
    fs::write(project_dir.join("settings.gradle"), settings_gradle).await?;
    
    // Create gradle.properties
    let gradle_properties = include_str!("../templates/gradle.properties");
    fs::write(project_dir.join("gradle.properties"), gradle_properties).await?;
    
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
    
    // Now ensure Gradle wrapper is initialized (after Gradle files exist)
    let gradle = ensure_gradle(&project_dir).await?;
    println!("Using Gradle: {}", gradle);
    
    // Create app/build.gradle
    let app_build_gradle = replace_template(include_str!("../templates/app_build.gradle"));
    fs::write(app_dir.join("build.gradle"), app_build_gradle).await?;
    
    // Create AndroidManifest.xml
    let manifest = replace_template(include_str!("../templates/AndroidManifest.xml"));
    fs::write(src_main_dir.join("AndroidManifest.xml"), manifest).await?;
    
    // Create NativeActivity Java class
    let native_activity = replace_template(include_str!("../templates/NativeActivity.java"));
    fs::write(java_dir.join("NativeActivity.java"), native_activity).await?;
    
    // Create strings.xml
    let strings_xml = replace_template(include_str!("../templates/strings.xml"));
    fs::write(res_dir.join("values").join("strings.xml"), strings_xml).await?;
    
    // Create Rust library
    let rust_cargo_toml = replace_template(include_str!("../templates/rust_Cargo.toml"));
    fs::write(rust_dir.join("Cargo.toml"), rust_cargo_toml).await?;
    
    // Create Rust lib.rs with ratatui example
    let rust_lib_rs = replace_template(include_str!("../templates/rust_lib.rs"));
    fs::write(rust_dir.join("src").join("lib.rs"), rust_lib_rs).await?;
    
    // Create Rust backend.rs (custom Ratatui backend)
    let rust_backend_rs = include_str!("../templates/rust_backend.rs");
    fs::write(rust_dir.join("src").join("backend.rs"), rust_backend_rs).await?;
    
    // Create Rust rasterizer.rs (software rasterizer)
    let rust_rasterizer_rs = include_str!("../templates/rust_rasterizer.rs");
    fs::write(rust_dir.join("src").join("rasterizer.rs"), rust_rasterizer_rs).await?;
    
    // Create Rust input.rs (Android input adapter)
    let rust_input_rs = include_str!("../templates/rust_input.rs");
    fs::write(rust_dir.join("src").join("input.rs"), rust_input_rs).await?;
    
    // Create Rust build.rs
    let rust_build_rs = include_str!("../templates/rust_build.rs");
    fs::write(rust_dir.join("build.rs"), rust_build_rs).await?;
    
    // Create fonts directory and copy font file if it exists
    let fonts_dir = rust_dir.join("fonts");
    fs::create_dir_all(&fonts_dir).await?;
    let template_font = Path::new("templates/fonts/Hack-Regular.ttf");
    if template_font.exists() {
        fs::copy(template_font, fonts_dir.join("Hack-Regular.ttf")).await?;
        println!("Copied font file to project");
    } else {
        // Create a README in fonts directory explaining how to add a font
        let font_readme = r#"# Fonts Directory

Place a monospace TrueType font (.ttf) file here named `Hack-Regular.ttf`.

You can download Hack font from: https://github.com/source-foundry/Hack/releases

Or use any other monospace font - just rename it to `Hack-Regular.ttf` or update the 
`include_bytes!` path in `src/lib.rs`.
"#;
        fs::write(fonts_dir.join("README.md"), font_readme).await?;
    }
    
    // Create .gitignore
    let gitignore = include_str!("../templates/gitignore");
    fs::write(project_dir.join(".gitignore"), gitignore).await?;
    
    // Create README.md
    let readme = replace_template(include_str!("../templates/README.md"));
    fs::write(project_dir.join("README.md"), readme).await?;
    
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
        println!("  3. ratadroid install       # Install on device/emulator");
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
    
    println!("Installing APK (variant: {})...", variant);
    
    let task = format!("install{}", capitalize_first(&variant));
    let status = Command::new(&gradle)
        .current_dir(&project_dir)
        .arg(&task)
        .status()?;
    
    if status.success() {
        println!("\n✓ Installation succeeded!");
    } else {
        return Err(format!("Installation failed with exit status {}", status.code().unwrap_or(-1)).into());
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
    
    // Then launch
    let package_name = format!("com.ratadroid.{}", 
        project_dir.file_name().and_then(|n| n.to_str()).unwrap_or("app"));
    
    println!("Launching app...");
    let status = Command::new(&adb)
        .args(["shell", "am", "start", "-n", &format!("{}/.NativeActivity", package_name)])
        .status();
    
    match status {
        Ok(s) if s.success() => println!("✓ App launched!"),
        Ok(_) => println!("⚠️  Launch command failed. Try manually: {} shell am start -n {}/.NativeActivity", adb, package_name),
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
