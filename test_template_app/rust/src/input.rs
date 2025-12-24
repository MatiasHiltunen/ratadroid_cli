//! Input adapter that converts Android input events to Ratatui/Crossterm events.
//! This allows TUI widgets to react to touch and keyboard input naturally.
//!
//! Uses Android's KeyCharacterMap for proper international keyboard support,
//! including combining accents for languages like French, Spanish, etc.

#[cfg(target_os = "android")]
use android_activity::input::{InputEvent, KeyMapChar, MotionAction, KeyAction, Keycode, MetaState};
#[cfg(target_os = "android")]
use android_activity::AndroidApp;
#[cfg(target_os = "android")]
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// State for handling combining accents (dead keys) for international keyboards
pub struct InputState {
    combining_accent: Option<char>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            combining_accent: None,
        }
    }
}

/// Converts raw Android Input to a Ratatui-compatible Event
/// Uses KeyCharacterMap for proper international keyboard support
pub fn map_android_event(
    event: &InputEvent,
    app: &AndroidApp,
    state: &mut InputState,
    font_width: f32,
    font_height: f32,
) -> Option<Event> {
    match event {
        InputEvent::KeyEvent(key) => {
            if key.action() == KeyAction::Down {
                let device_id = key.device_id();
                map_key_code_with_character_map(key.key_code(), key.meta_state(), device_id, app, state)
                    .map(Event::Key)
            } else {
                None
            }
        }
        InputEvent::MotionEvent(motion) => {
            // Only handle the first pointer (single touch) for simplicity
            let action = motion.action();
            let pointer = motion.pointers().next()?;
            
            // Map Pixel coordinates to Grid coordinates
            let col = (pointer.x() / font_width) as u16;
            let row = (pointer.y() / font_height) as u16;

            let kind = match action {
                MotionAction::Down => MouseEventKind::Down(MouseButton::Left),
                MotionAction::Up => MouseEventKind::Up(MouseButton::Left),
                MotionAction::Move => MouseEventKind::Drag(MouseButton::Left),
                _ => return None,
            };

            Some(Event::Mouse(MouseEvent {
                kind,
                column: col,
                row,
                modifiers: KeyModifiers::empty(),
            }))
        }
        _ => None,
    }
}

/// Maps Android keycode to Ratatui KeyEvent using KeyCharacterMap for international support
fn map_key_code_with_character_map(
    key_code: Keycode,
    meta_state: MetaState,
    device_id: i32,
    app: &AndroidApp,
    state: &mut InputState,
) -> Option<KeyEvent> {
    // First, try to get the Unicode character from KeyCharacterMap
    // This handles international keyboards properly
    if let Ok(key_map) = app.device_key_character_map(device_id) {
        match key_map.get(key_code, meta_state) {
            Ok(KeyMapChar::Unicode(ch)) => {
                // Handle combining accents (dead keys)
                let final_char = if let Some(accent) = state.combining_accent {
                    // Try to combine the accent with the character
                    match key_map.get_dead_char(accent, ch) {
                        Ok(Some(combined)) => {
                            state.combining_accent = None;
                            combined
                        }
                        Ok(None) => {
                            // Can't combine, use the character as-is
                            state.combining_accent = None;
                            ch
                        }
                        Err(_) => {
                            // Error combining, use the character as-is
                            state.combining_accent = None;
                            ch
                        }
                    }
                } else {
                    ch
                };
                
                // Map modifiers
                let mut modifiers = KeyModifiers::empty();
                if meta_state.shift_on() {
                    modifiers |= KeyModifiers::SHIFT;
                }
                if meta_state.ctrl_on() {
                    modifiers |= KeyModifiers::CONTROL;
                }
                if meta_state.alt_on() {
                    modifiers |= KeyModifiers::ALT;
                }
                
                return Some(KeyEvent::new(KeyCode::Char(final_char), modifiers));
            }
            Ok(KeyMapChar::CombiningAccent(accent)) => {
                // Store the combining accent for the next key press
                state.combining_accent = Some(accent);
                return None; // Don't emit an event for the accent key itself
            }
            Ok(KeyMapChar::None) => {
                // Not a Unicode character, fall through to special key mapping
                state.combining_accent = None;
            }
            Err(_) => {
                // Error getting character map, fall through to special key mapping
                state.combining_accent = None;
            }
        }
    }
    
    // Fallback: Map special keys that don't have Unicode characters
    let code = match key_code {
        // Special keys
        Keycode::Enter => KeyCode::Enter,
        Keycode::Space => KeyCode::Char(' '),
        Keycode::Del => KeyCode::Backspace,
        Keycode::Escape | Keycode::Back => KeyCode::Esc,
        Keycode::DpadUp => KeyCode::Up,
        Keycode::DpadDown => KeyCode::Down,
        Keycode::DpadLeft => KeyCode::Left,
        Keycode::DpadRight => KeyCode::Right,
        Keycode::Tab => KeyCode::Tab,
        Keycode::PageUp => KeyCode::PageUp,
        Keycode::PageDown => KeyCode::PageDown,
        Keycode::Home => KeyCode::Home,
        Keycode::Insert => KeyCode::Insert,
        _ => return None,
    };

    // Map modifiers
    let mut modifiers = KeyModifiers::empty();
    if meta_state.shift_on() {
        modifiers |= KeyModifiers::SHIFT;
    }
    if meta_state.ctrl_on() {
        modifiers |= KeyModifiers::CONTROL;
    }
    if meta_state.alt_on() {
        modifiers |= KeyModifiers::ALT;
    }

    Some(KeyEvent::new(code, modifiers))
}

