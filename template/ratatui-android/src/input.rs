//! Input handling utilities for Android TUI applications.
//!
//! Provides types and functions for converting touch events and keyboard
//! key names to crossterm events.
//!
//! For Android soft keyboard input, international characters (Finnish, Swedish, etc.)
//! are typically delivered through InputConnection.commitText() rather than as
//! individual key events. This module handles the key events that ARE delivered.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

#[cfg(target_os = "android")]
use android_activity::input::{Keycode, MetaState};

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

/// Convert Android keycode and meta state to a crossterm KeyEvent.
///
/// This handles key events from Android's native input queue, including:
/// - Navigation keys (arrows, Enter, Tab, etc.)
/// - Letters (with shift for uppercase)
/// - Numbers and punctuation
///
/// Note: International characters (ä, ö, å) from soft keyboards typically
/// come through InputConnection.commitText(), not as key events.
#[cfg(target_os = "android")]
pub fn android_keycode_to_event(key_code: Keycode, meta: MetaState) -> Option<KeyEvent> {
    let shift = meta.shift_on();
    
    let key = match key_code {
        // Navigation and control keys
        Keycode::Enter => KeyCode::Enter,
        Keycode::Escape => KeyCode::Esc,
        Keycode::Tab => KeyCode::Tab,
        Keycode::Del => KeyCode::Backspace,
        Keycode::ForwardDel => KeyCode::Delete,
        Keycode::DpadUp => KeyCode::Up,
        Keycode::DpadDown => KeyCode::Down,
        Keycode::DpadLeft => KeyCode::Left,
        Keycode::DpadRight => KeyCode::Right,
        Keycode::Home => KeyCode::Home,
        Keycode::MoveEnd => KeyCode::End,
        Keycode::PageUp => KeyCode::PageUp,
        Keycode::PageDown => KeyCode::PageDown,
        
        // Letters (handle shift for uppercase)
        Keycode::A => KeyCode::Char(if shift { 'A' } else { 'a' }),
        Keycode::B => KeyCode::Char(if shift { 'B' } else { 'b' }),
        Keycode::C => KeyCode::Char(if shift { 'C' } else { 'c' }),
        Keycode::D => KeyCode::Char(if shift { 'D' } else { 'd' }),
        Keycode::E => KeyCode::Char(if shift { 'E' } else { 'e' }),
        Keycode::F => KeyCode::Char(if shift { 'F' } else { 'f' }),
        Keycode::G => KeyCode::Char(if shift { 'G' } else { 'g' }),
        Keycode::H => KeyCode::Char(if shift { 'H' } else { 'h' }),
        Keycode::I => KeyCode::Char(if shift { 'I' } else { 'i' }),
        Keycode::J => KeyCode::Char(if shift { 'J' } else { 'j' }),
        Keycode::K => KeyCode::Char(if shift { 'K' } else { 'k' }),
        Keycode::L => KeyCode::Char(if shift { 'L' } else { 'l' }),
        Keycode::M => KeyCode::Char(if shift { 'M' } else { 'm' }),
        Keycode::N => KeyCode::Char(if shift { 'N' } else { 'n' }),
        Keycode::O => KeyCode::Char(if shift { 'O' } else { 'o' }),
        Keycode::P => KeyCode::Char(if shift { 'P' } else { 'p' }),
        Keycode::Q => KeyCode::Char(if shift { 'Q' } else { 'q' }),
        Keycode::R => KeyCode::Char(if shift { 'R' } else { 'r' }),
        Keycode::S => KeyCode::Char(if shift { 'S' } else { 's' }),
        Keycode::T => KeyCode::Char(if shift { 'T' } else { 't' }),
        Keycode::U => KeyCode::Char(if shift { 'U' } else { 'u' }),
        Keycode::V => KeyCode::Char(if shift { 'V' } else { 'v' }),
        Keycode::W => KeyCode::Char(if shift { 'W' } else { 'w' }),
        Keycode::X => KeyCode::Char(if shift { 'X' } else { 'x' }),
        Keycode::Y => KeyCode::Char(if shift { 'Y' } else { 'y' }),
        Keycode::Z => KeyCode::Char(if shift { 'Z' } else { 'z' }),
        
        // Numbers
        Keycode::Keycode0 => KeyCode::Char('0'),
        Keycode::Keycode1 => KeyCode::Char('1'),
        Keycode::Keycode2 => KeyCode::Char('2'),
        Keycode::Keycode3 => KeyCode::Char('3'),
        Keycode::Keycode4 => KeyCode::Char('4'),
        Keycode::Keycode5 => KeyCode::Char('5'),
        Keycode::Keycode6 => KeyCode::Char('6'),
        Keycode::Keycode7 => KeyCode::Char('7'),
        Keycode::Keycode8 => KeyCode::Char('8'),
        Keycode::Keycode9 => KeyCode::Char('9'),
        
        // Space and common punctuation
        Keycode::Space => KeyCode::Char(' '),
        Keycode::Comma => KeyCode::Char(','),
        Keycode::Period => KeyCode::Char('.'),
        Keycode::Minus => KeyCode::Char('-'),
        Keycode::Equals => KeyCode::Char('='),
        Keycode::LeftBracket => KeyCode::Char('['),
        Keycode::RightBracket => KeyCode::Char(']'),
        Keycode::Backslash => KeyCode::Char('\\'),
        Keycode::Semicolon => KeyCode::Char(';'),
        Keycode::Apostrophe => KeyCode::Char('\''),
        Keycode::Slash => KeyCode::Char('/'),
        Keycode::At => KeyCode::Char('@'),
        Keycode::Plus => KeyCode::Char('+'),
        Keycode::Star => KeyCode::Char('*'),
        Keycode::Pound => KeyCode::Char('#'),
        Keycode::Grave => KeyCode::Char('`'),
        
        // Unknown keycodes - log for debugging
        _ => {
            #[cfg(target_os = "android")]
            log::debug!("Unknown Android keycode: {:?}", key_code);
            return None;
        }
    };
    
    // Build modifiers
    let mut modifiers = KeyModifiers::empty();
    if meta.ctrl_on() {
        modifiers |= KeyModifiers::CONTROL;
    }
    if meta.alt_on() {
        modifiers |= KeyModifiers::ALT;
    }
    // Note: SHIFT is already handled above for character case
    
    Some(KeyEvent::new(key, modifiers))
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

