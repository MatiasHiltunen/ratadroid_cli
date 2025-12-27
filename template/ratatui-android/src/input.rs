//! Input handling utilities for Android TUI applications.
//!
//! Provides types and functions for converting touch events and keyboard
//! key names to crossterm events.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

/// Touch action types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    /// Finger touched the screen
    Down,
    /// Finger lifted from screen
    Up,
    /// Finger moved on screen
    Move,
    /// Touch cancelled
    Cancel,
}

/// A touch event with screen coordinates.
#[derive(Debug, Clone)]
pub struct TouchEvent {
    /// Touch action type
    pub action: TouchAction,
    /// X coordinate in pixels
    pub x: f32,
    /// Y coordinate in pixels
    pub y: f32,
    /// Pointer ID (for multi-touch)
    pub pointer_id: i32,
}

impl TouchEvent {
    /// Create a new touch event.
    pub fn new(action: TouchAction, x: f32, y: f32, pointer_id: i32) -> Self {
        Self { action, x, y, pointer_id }
    }

    /// Convert pixel coordinates to terminal cell coordinates.
    pub fn to_terminal_coords(&self, font_width: f32, font_height: f32, top_offset_px: f32) -> (u16, u16) {
        let col = (self.x / font_width) as u16;
        let row = ((self.y - top_offset_px).max(0.0) / font_height) as u16;
        (col, row)
    }
}

/// Convert a keyboard key name to a crossterm event.
///
/// This converts key names from the on-screen keyboard (like "ESC", "TAB", "UP")
/// to crossterm `Event` values that can be processed by the TUI application.
///
/// # Arguments
///
/// * `key_name` - The key name (e.g., "ESC", "TAB", "ENTER")
/// * `shift_active` - Whether Shift modifier is active
/// * `ctrl_active` - Whether Ctrl modifier is active
///
/// # Returns
///
/// `Some(Event)` if the key name is recognized, `None` otherwise.
///
/// # Example
///
/// ```rust
/// use ratatui_android::input::key_to_crossterm_event;
/// use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
///
/// let event = key_to_crossterm_event("ENTER", false, false);
/// assert!(event.is_some());
/// ```
pub fn key_to_crossterm_event(key_name: &str, shift_active: bool, ctrl_active: bool) -> Option<Event> {
    let mut modifiers = KeyModifiers::empty();
    if shift_active {
        modifiers |= KeyModifiers::SHIFT;
    }
    if ctrl_active {
        modifiers |= KeyModifiers::CONTROL;
    }

    let key_code = match key_name {
        "ESC" => KeyCode::Esc,
        "TAB" => KeyCode::Tab,
        "SHIFT" | "CTRL" => {
            // Toggle keys - no event needed, handled by state
            return None;
        }
        "UP" => KeyCode::Up,
        "DOWN" => KeyCode::Down,
        "LEFT" => KeyCode::Left,
        "RIGHT" => KeyCode::Right,
        "ENTER" => KeyCode::Enter,
        "SPACE" => KeyCode::Char(' '),
        "BACKSPACE" => KeyCode::Backspace,
        "DELETE" => KeyCode::Delete,
        "KEYBOARD" => {
            // Keyboard toggle - handled separately
            return None;
        }
        // Single character keys
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            KeyCode::Char(c)
        }
        _ => return None,
    };

    Some(Event::Key(KeyEvent::new(key_code, modifiers)))
}

/// Standard keyboard key names.
pub mod keys {
    /// Escape key
    pub const ESC: &str = "ESC";
    /// Tab key
    pub const TAB: &str = "TAB";
    /// Shift modifier
    pub const SHIFT: &str = "SHIFT";
    /// Control modifier
    pub const CTRL: &str = "CTRL";
    /// Up arrow
    pub const UP: &str = "UP";
    /// Down arrow
    pub const DOWN: &str = "DOWN";
    /// Left arrow
    pub const LEFT: &str = "LEFT";
    /// Right arrow
    pub const RIGHT: &str = "RIGHT";
    /// Enter/Return key
    pub const ENTER: &str = "ENTER";
    /// Space key
    pub const SPACE: &str = "SPACE";
    /// Backspace key
    pub const BACKSPACE: &str = "BACKSPACE";
    /// Delete key
    pub const DELETE: &str = "DELETE";
    /// Keyboard toggle (show/hide software keyboard)
    pub const KEYBOARD: &str = "KEYBOARD";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_to_crossterm_event() {
        let event = key_to_crossterm_event("ENTER", false, false);
        assert!(event.is_some());

        let event = key_to_crossterm_event("ESC", false, false);
        assert!(event.is_some());

        // Toggle keys return None
        let event = key_to_crossterm_event("SHIFT", false, false);
        assert!(event.is_none());

        // Unknown key returns None
        let event = key_to_crossterm_event("UNKNOWN", false, false);
        assert!(event.is_none());
    }

    #[test]
    fn test_key_with_modifiers() {
        let event = key_to_crossterm_event("a", true, false);
        if let Some(Event::Key(key_event)) = event {
            assert!(key_event.modifiers.contains(KeyModifiers::SHIFT));
        } else {
            panic!("Expected Key event");
        }
    }

    #[test]
    fn test_touch_event() {
        let touch = TouchEvent::new(TouchAction::Down, 100.0, 200.0, 0);
        let (col, row) = touch.to_terminal_coords(30.0, 48.0, 48.0);
        assert_eq!(col, 3); // 100 / 30 = 3
        assert_eq!(row, 3); // (200 - 48) / 48 = 3
    }
}

