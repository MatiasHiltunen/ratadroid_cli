//! Ratadroid - Android Runtime for Ratatui TUI Applications
//!
//! This crate provides a generic Android runtime for Ratatui TUI applications.
//! Apps implement the `RatadroidApp` trait and register via `set_app_factory`.
//!
//! # Example
//!
//! ```rust,ignore
//! use ratadroid::{RatadroidApp, RatadroidContext, set_app_factory};
//! use ratatui::Frame;
//! use crossterm::event::Event;
//!
//! struct MyApp;
//!
//! impl RatadroidApp for MyApp {
//!     fn draw(&mut self, frame: &mut Frame, ctx: &RatadroidContext) {
//!         // Draw your UI here
//!     }
//!
//!     fn handle_event(&mut self, event: Event, ctx: &mut RatadroidContext) {
//!         // Handle input events
//!     }
//! }
//!
//! // Register app factory before android_main runs
//! set_app_factory(|| Box::new(MyApp));
//! ```

// Re-export ratatui-android types for convenience
pub use ratatui_android::{
    AndroidBackend, Rasterizer, DirectKeyboard, DirectKeyboardState, KeyboardState,
    warm_cache, is_wide_char,
};

// Re-export ratatui for convenience
pub use ratatui;

#[cfg(target_os = "android")]
pub mod jni_utils;

#[cfg(target_os = "android")]
mod runtime;

#[cfg(target_os = "android")]
pub use runtime::*;

// Demo app for testing
pub mod demo;

#[cfg(target_os = "android")]
use std::sync::Mutex;

#[cfg(target_os = "android")]
use crossterm::event::Event as CrosstermEvent;

/// Context passed to the app during draw and event handling
#[cfg(target_os = "android")]
pub struct RatadroidContext {
    /// Request the app to quit
    pub should_quit: bool,
    /// Request a redraw
    pub needs_draw: bool,
    /// Android data directory path
    pub data_dir: std::path::PathBuf,
    /// Current screen orientation
    pub orientation: Orientation,
    /// Screen dimensions in terminal cells
    pub cols: u16,
    pub rows: u16,
    /// Font dimensions
    pub font_width: f32,
    pub font_height: f32,
}

#[cfg(target_os = "android")]
impl RatadroidContext {
    /// Request the app to quit
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Request a redraw on the next frame
    pub fn request_redraw(&mut self) {
        self.needs_draw = true;
    }
}

/// Screen orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Trait that apps must implement to run on Android
#[cfg(target_os = "android")]
pub trait RatadroidApp: Send {
    /// Initialize the app. Called once after Android context is available.
    /// Return an error to abort startup.
    fn init(&mut self, _ctx: &RatadroidContext) -> anyhow::Result<()> {
        Ok(())
    }

    /// Draw the app's UI. Called on each frame.
    fn draw(&mut self, frame: &mut ratatui::Frame, ctx: &RatadroidContext);

    /// Handle an input event. Return true if the event was consumed.
    fn handle_event(&mut self, event: CrosstermEvent, ctx: &mut RatadroidContext) -> bool;

    /// Called when the screen is resized.
    fn on_resize(&mut self, _cols: u16, _rows: u16, _ctx: &RatadroidContext) {}

    /// Called periodically (roughly every tick). Use for async operations.
    fn tick(&mut self, _ctx: &mut RatadroidContext) {}

    /// App name for logging
    fn name(&self) -> &str {
        "RatadroidApp"
    }
}

/// Factory function type for creating app instances
#[cfg(target_os = "android")]
pub type AppFactory = fn() -> Box<dyn RatadroidApp>;

/// Global app factory - set this before android_main runs
#[cfg(target_os = "android")]
static APP_FACTORY: Mutex<Option<AppFactory>> = Mutex::new(None);

/// Set the app factory function. Must be called before android_main.
#[cfg(target_os = "android")]
pub fn set_app_factory(factory: AppFactory) {
    if let Ok(mut guard) = APP_FACTORY.lock() {
        *guard = Some(factory);
    }
}

/// Get the app factory (internal use)
#[cfg(target_os = "android")]
pub(crate) fn get_app_factory() -> Option<AppFactory> {
    APP_FACTORY.lock().ok()?.clone()
}

// Non-Android stubs for compilation
#[cfg(not(target_os = "android"))]
pub struct RatadroidContext {
    pub should_quit: bool,
    pub needs_draw: bool,
    pub data_dir: std::path::PathBuf,
    pub orientation: Orientation,
    pub cols: u16,
    pub rows: u16,
    pub font_width: f32,
    pub font_height: f32,
}

#[cfg(not(target_os = "android"))]
impl RatadroidContext {
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn request_redraw(&mut self) {
        self.needs_draw = true;
    }
}

#[cfg(not(target_os = "android"))]
pub trait RatadroidApp: Send {
    fn init(&mut self, _ctx: &RatadroidContext) -> anyhow::Result<()> { Ok(()) }
    fn draw(&mut self, frame: &mut ratatui::Frame, ctx: &RatadroidContext);
    fn handle_event(&mut self, event: crossterm::event::Event, ctx: &mut RatadroidContext) -> bool;
    fn on_resize(&mut self, _cols: u16, _rows: u16, _ctx: &RatadroidContext) {}
    fn tick(&mut self, _ctx: &mut RatadroidContext) {}
    fn name(&self) -> &str { "RatadroidApp" }
}

#[cfg(not(target_os = "android"))]
pub fn set_app_factory(_factory: fn() -> Box<dyn RatadroidApp>) {}

