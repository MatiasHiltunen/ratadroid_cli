//! # ratatui-android
//!
//! Android backend for [Ratatui](https://ratatui.rs/) - enables TUI applications
//! to run on Android devices with full touch support.
//!
//! ## Features
//!
//! - **Software Rasterizer**: Converts Ratatui's cell grid to pixels using cosmic-text
//! - **Touch Input**: Maps touch events to terminal key events  
//! - **On-Screen Keyboard**: Built-in virtual keyboard for special keys
//! - **Unicode Support**: Full emoji and CJK character rendering
//! - **Unicode Rendering**: Full emoji and CJK character rendering via cosmic-text
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ratatui_android::{AndroidBackend, Rasterizer};
//! use ratatui::Terminal;
//!
//! // Create rasterizer with desired font size
//! let rasterizer = Rasterizer::new(48.0);
//!
//! // Create backend with terminal dimensions
//! let backend = AndroidBackend::new(80, 24);
//!
//! // Create Ratatui terminal
//! let mut terminal = Terminal::new(backend)?;
//!
//! // Draw your UI
//! terminal.draw(|frame| {
//!     // Your UI code here
//! })?;
//!
//! // Render to pixel buffer
//! let mut pixels = vec![0u8; width * height * 4];
//! rasterizer.render_to_surface(terminal.backend(), &mut pixels, stride, width, height);
//! ```
//!
//! ## Feature Flags
//!
//! - `swash-backend`: Enable swash for emoji fallback rendering
//! - `ab-glyph-backend`: Enable ab_glyph for text fallback rendering
//!
//! ## Integration Guide
//!
//! See the [README](https://github.com/LucasPickering/slumber/blob/main/crates/ratatui-android/README.md)
//! for detailed integration instructions.

mod backend;
mod rasterizer;

#[cfg(all(target_os = "android", feature = "android-native-render"))]
mod android_render;

pub mod widgets;
pub mod input;

// Re-export main types
pub use backend::AndroidBackend;
pub use rasterizer::{Rasterizer, CachedChar, is_wide_char, is_emoji_or_special, warm_cache, CHAR_CACHE};

// Re-export widget types
pub use widgets::{
    DirectKeyboard, 
    DirectKeyboardState,
    KeyboardWidget,
    KeyboardState,
};

// Re-export input utilities
pub use input::{TouchEvent, TouchAction, key_to_crossterm_event};

#[cfg(target_os = "android")]
pub use input::android_keycode_to_event;

/// Configuration for the Android backend
#[derive(Clone, Debug)]
pub struct AndroidConfig {
    /// Font size in pixels (default: 48.0)
    pub font_size: f32,
    
    /// Height of the on-screen keyboard in pixels (default: 80)
    pub keyboard_height: u32,
    
    /// Status bar height in pixels (queried from Android if 0)
    pub status_bar_height: u32,
    
    /// Navigation bar height in pixels (queried from Android if 0)
    pub nav_bar_height: u32,
    
    /// Whether to warm the character cache at startup
    pub warm_cache: bool,
}

impl Default for AndroidConfig {
    fn default() -> Self {
        Self {
            font_size: 48.0,
            keyboard_height: 80,
            status_bar_height: 0,
            nav_bar_height: 0,
            warm_cache: true,
        }
    }
}

/// Screen layout information
#[derive(Clone, Debug, Default)]
pub struct ScreenLayout {
    /// Total screen width in pixels
    pub width_px: u32,
    
    /// Total screen height in pixels
    pub height_px: u32,
    
    /// Visible height (excluding soft keyboard)
    pub visible_height_px: u32,
    
    /// Terminal columns
    pub cols: u16,
    
    /// Terminal rows
    pub rows: u16,
    
    /// Top offset in rows (status bar)
    pub top_offset_rows: u16,
    
    /// Bottom offset in rows (navigation bar + keyboard)
    pub bottom_offset_rows: u16,
    
    /// Font width in pixels
    pub font_width: f32,
    
    /// Font height in pixels
    pub font_height: f32,
}

impl ScreenLayout {
    /// Calculate layout from screen dimensions and config
    pub fn calculate(
        screen_width: u32,
        screen_height: u32,
        visible_height: u32,
        config: &AndroidConfig,
        rasterizer: &Rasterizer,
    ) -> Self {
        let font_width = rasterizer.font_width();
        let font_height = rasterizer.font_height();
        
        let cols = (screen_width as f32 / font_width) as u16;
        let total_rows = (visible_height as f32 / font_height) as u16;
        
        // Calculate offsets
        let status_bar_rows = if config.status_bar_height > 0 {
            ((config.status_bar_height as f32 / font_height).ceil() as u16).max(1)
        } else {
            1 // Default minimum
        };
        
        let keyboard_rows = ((config.keyboard_height as f32 / font_height).ceil() as u16).max(2);
        let nav_bar_rows = if config.nav_bar_height > 0 {
            ((config.nav_bar_height as f32 / font_height).ceil() as u16).max(1)
        } else {
            1 // Default minimum
        };
        
        let top_offset_rows = status_bar_rows.min(total_rows / 4);
        let bottom_offset_rows = keyboard_rows + nav_bar_rows;
        
        let available_rows = total_rows
            .saturating_sub(top_offset_rows)
            .saturating_sub(bottom_offset_rows);
        
        Self {
            width_px: screen_width,
            height_px: screen_height,
            visible_height_px: visible_height,
            cols,
            rows: available_rows,
            top_offset_rows,
            bottom_offset_rows,
            font_width,
            font_height,
        }
    }
    
    /// Get the pixel Y position where the direct keyboard starts
    pub fn keyboard_y(&self, nav_bar_height_px: u32, keyboard_height_px: u32) -> usize {
        (self.height_px as usize)
            .saturating_sub(keyboard_height_px as usize)
            .saturating_sub(nav_bar_height_px as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AndroidConfig::default();
        assert_eq!(config.font_size, 48.0);
        assert_eq!(config.keyboard_height, 80);
    }

    #[test]
    fn test_screen_layout_calculation() {
        let config = AndroidConfig {
            font_size: 48.0,
            keyboard_height: 80,
            status_bar_height: 48,
            nav_bar_height: 48,
            warm_cache: false,
        };
        
        let rasterizer = Rasterizer::new(48.0);
        let layout = ScreenLayout::calculate(1080, 1920, 1920, &config, &rasterizer);
        
        assert!(layout.cols > 0);
        assert!(layout.rows > 0);
        assert!(layout.top_offset_rows > 0);
        assert!(layout.bottom_offset_rows > 0);
    }
}

