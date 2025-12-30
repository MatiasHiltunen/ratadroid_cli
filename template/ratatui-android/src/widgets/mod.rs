//! Widget components for the Android backend.
//!
//! This module provides widgets that are useful for Android TUI applications:
//!
//! - [`KeyboardWidget`] - A Ratatui widget for rendering an on-screen keyboard
//! - [`DirectKeyboard`] - A direct-to-pixel keyboard renderer (bypasses Ratatui)
//! - [`KeyboardState`] - State management for the Ratatui keyboard widget
//! - [`DirectKeyboardState`] - State management for the direct keyboard

mod keyboard;
mod direct_keyboard;

pub use keyboard::{KeyboardWidget, KeyboardState};
pub use direct_keyboard::{DirectKeyboard, DirectKeyboardState};

