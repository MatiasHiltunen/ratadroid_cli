//! Input adapter that converts Android input events to Ratatui/Crossterm events.
//! This allows TUI widgets to react to touch and keyboard input naturally.
//!
//! Uses Android's KeyCharacterMap for proper international keyboard support,
//! including combining accents for languages like French, Spanish, etc.
//! 
//! Special support for Scandinavian keyboards (Swedish, Norwegian, Danish, Finnish, Icelandic):
//! - Handles characters like å, ä, ö, æ, ø, and their uppercase variants
//! - Supports AltGr (Right Alt) combinations
//! - Handles dead keys and combining accents properly

#[cfg(target_os = "android")]
use android_activity::input::{InputEvent, KeyMapChar, MotionAction, KeyAction, Keycode, MetaState};
#[cfg(target_os = "android")]
use log;
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
    top_offset_px: f32, // Top offset in pixels (for status bar)
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
            // Subtract top offset to account for status bar
            let adjusted_y = (pointer.y() - top_offset_px).max(0.0);
            let col = (pointer.x() / font_width) as u16;
            let row = (adjusted_y / font_height) as u16;

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
/// Supports Scandinavian keyboards (Swedish, Norwegian, Danish, Finnish, Icelandic)
fn map_key_code_with_character_map(
    key_code: Keycode,
    meta_state: MetaState,
    device_id: i32,
    app: &AndroidApp,
    state: &mut InputState,
) -> Option<KeyEvent> {
    // First, try to get the Unicode character from KeyCharacterMap
    // This handles international keyboards properly, including Scandinavian layouts
    if let Ok(key_map) = app.device_key_character_map(device_id) {
        // Log all key presses for debugging Scandinavian characters
        log::info!("Key pressed: keycode={:?}, meta_state: alt={}, shift={}, ctrl={}", 
                   key_code, meta_state.alt_on(), meta_state.shift_on(), meta_state.ctrl_on());
        
        // Try multiple meta state combinations
        // Virtual keyboards often send Alt+key for special characters, but KeyCharacterMap
        // might need to be queried without Alt to get the actual character
        let mut result = key_map.get(key_code, meta_state);
        
        // If we got None and Alt is pressed, try alternative approaches
        // Physical Finnish keyboards use AltGr (Right Alt) which appears as Alt+key
        // KeyCharacterMap often returns None for these combinations, so we need a fallback
        if matches!(result, Ok(KeyMapChar::None)) && meta_state.alt_on() {
            // Try with virtual keyboard device ID (-1) which might have different mappings
            if let Ok(vk_key_map) = app.device_key_character_map(-1) {
                let vk_result = vk_key_map.get(key_code, meta_state);
                if !matches!(vk_result, Ok(KeyMapChar::None)) {
                    log::info!("Virtual keyboard key map returned different result");
                    result = vk_result;
                }
            }
            
            // Fallback mapping for Finnish/Scandinavian keyboards
            // Physical keyboards use AltGr (Right Alt) which sends Alt+key combinations
            // Based on user feedback: Alt+P = ä, Alt+Q = å, Alt+W = ö
            let scandinavian_char = match (key_code, meta_state.shift_on()) {
                (Keycode::P, false) => Some('ä'),
                (Keycode::P, true) => Some('Ä'),
                (Keycode::Q, false) => Some('å'),
                (Keycode::Q, true) => Some('Å'),
                (Keycode::W, false) => Some('ö'),
                (Keycode::W, true) => Some('Ö'),
                // Additional Finnish keyboard mappings if needed
                (Keycode::A, false) if meta_state.alt_on() => Some('ä'), // Some layouts
                (Keycode::A, true) if meta_state.alt_on() => Some('Ä'),
                (Keycode::O, false) if meta_state.alt_on() => Some('ö'), // Some layouts
                (Keycode::O, true) if meta_state.alt_on() => Some('Ö'),
                _ => None,
            };
            
            if let Some(ch) = scandinavian_char {
                log::info!("Using fallback mapping for Finnish keyboard: keycode={:?} (Alt={}, Shift={}) -> '{}'", 
                          key_code, meta_state.alt_on(), meta_state.shift_on(), ch);
                // Return the character directly without going through KeyCharacterMap
                let mut modifiers = KeyModifiers::empty();
                if meta_state.shift_on() {
                    modifiers |= KeyModifiers::SHIFT;
                }
                return Some(KeyEvent::new(KeyCode::Char(ch), modifiers));
            }
        }
        
        match result {
            Ok(KeyMapChar::Unicode(ch)) => {
                log::info!("KeyCharacterMap returned Unicode: '{}' (U+{:04X})", ch, ch as u32);
                // Handle combining accents (dead keys) - important for Scandinavian keyboards
                // Some Scandinavian layouts use dead keys for certain characters
                let final_char = if let Some(accent) = state.combining_accent {
                    // Try to combine the accent with the character
                    match key_map.get_dead_char(accent, ch) {
                        Ok(Some(combined)) => {
                            state.combining_accent = None;
                            combined
                        }
                        Ok(None) => {
                            // Can't combine, use the character as-is
                            // This handles cases like pressing a dead key then a non-combinable character
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
                
                // Map modifiers - important for AltGr (Right Alt) on Scandinavian keyboards
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
                
                // Log Scandinavian characters for debugging
                if matches!(final_char, 'å' | 'ä' | 'ö' | 'æ' | 'ø' | 'Å' | 'Ä' | 'Ö' | 'Æ' | 'Ø') {
                    log::debug!("Scandinavian character detected: '{}' (keycode: {:?}, meta_state: alt={}, shift={})", 
                               final_char, key_code, meta_state.alt_on(), meta_state.shift_on());
                }
                
                return Some(KeyEvent::new(KeyCode::Char(final_char), modifiers));
            }
            Ok(KeyMapChar::CombiningAccent(accent)) => {
                // Store the combining accent for the next key press
                // This is used for dead keys common in Scandinavian keyboards
                log::info!("Combining accent detected: '{}' (keycode: {:?})", accent, key_code);
                state.combining_accent = Some(accent);
                return None; // Don't emit an event for the accent key itself
            }
            Ok(KeyMapChar::None) => {
                // Not a Unicode character, fall through to special key mapping
                // This might happen for Scandinavian characters if KeyCharacterMap doesn't work
                log::warn!("KeyCharacterMap returned None for keycode: {:?} (meta_state: alt={}, shift={}, ctrl={})", 
                          key_code, meta_state.alt_on(), meta_state.shift_on(), meta_state.ctrl_on());
                
                // Clear any pending combining accent
                if state.combining_accent.is_some() {
                    log::debug!("KeyMapChar::None received, clearing combining accent");
                    state.combining_accent = None;
                }
            }
            Err(e) => {
                // Error getting character map, fall through to special key mapping
                log::warn!("Error getting character from KeyCharacterMap: {:?} (keycode: {:?}, meta_state: alt={}, shift={})", 
                          e, key_code, meta_state.alt_on(), meta_state.shift_on());
                state.combining_accent = None;
            }
        }
    }
    
    // Fallback: Map special keys and common Scandinavian characters
    // This provides fallback support if KeyCharacterMap doesn't work
    let code = match key_code {
        // Special keys
        Keycode::Enter => {
            log::info!("Enter key pressed (keycode: {:?})", key_code);
            KeyCode::Enter
        },
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
        _ => {
            // If we have a combining accent stored, try to use it
            if let Some(accent) = state.combining_accent {
                // Try to combine with a character if possible
                // This handles cases where KeyCharacterMap might not work perfectly
                log::info!("Combining accent '{}' stored but no character to combine with", accent);
                state.combining_accent = None;
                // Return None - let the next key press handle it
                return None;
            }
            log::warn!("Unhandled keycode: {:?} (meta_state: alt={}, shift={}, ctrl={})", 
                      key_code, meta_state.alt_on(), meta_state.shift_on(), meta_state.ctrl_on());
            return None;
        }
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

