//! Input event processing and conversion

use android_activity::{AndroidApp, input::InputEvent};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent as CrosstermKeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui_android::android_keycode_to_event;

/// Map Android input events to crossterm events
/// Returns None if the event is invalid or should be ignored
pub fn map_android_event(
    event: &InputEvent,
    _app: &AndroidApp,
    font_width: f32,
    font_height: f32,
    top_offset_px: f32,
    bottom_offset_px: f32,
    window_height: f32,
) -> Option<CrosstermEvent> {
    use android_activity::input::{KeyAction, MotionAction};
    
    // Validate font dimensions
    if font_width <= 0.0 || font_height <= 0.0 {
        return None;
    }
    
    match event {
        InputEvent::KeyEvent(key) => {
            if key.action() == KeyAction::Down {
                // Use the shared function from ratatui-android input module
                android_keycode_to_event(key.key_code(), key.meta_state())
                    .map(CrosstermEvent::Key)
            } else {
                None
            }
        }
        InputEvent::MotionEvent(motion) => {
            let action = motion.action();
            let pointer = motion.pointers().next()?;
            
            // Validate coordinates
            let x = pointer.x();
            let y = pointer.y();
            
            // Check for invalid coordinates
            if !validate_coordinates(x, y, window_height, window_height) {
                return None;
            }
            
            let adjusted_y = (y - top_offset_px).max(0.0);
            let max_content_y = window_height - bottom_offset_px;
            if y >= max_content_y {
                return None;
            }
            
            // Calculate cell coordinates with bounds checking
            let col = (x / font_width) as u16;
            let row = (adjusted_y / font_height) as u16;
            
            // Validate calculated coordinates
            if col > 1000 || row > 1000 {
                // Sanity check - coordinates seem invalid
                return None;
            }
            
            let kind = match action {
                MotionAction::Down => MouseEventKind::Down(MouseButton::Left),
                MotionAction::Up => MouseEventKind::Up(MouseButton::Left),
                MotionAction::Move => MouseEventKind::Drag(MouseButton::Left),
                // Handle secondary/tertiary buttons and other actions to prevent ANR
                MotionAction::Cancel => MouseEventKind::Up(MouseButton::Left),
                MotionAction::PointerDown => MouseEventKind::Down(MouseButton::Left),
                MotionAction::PointerUp => MouseEventKind::Up(MouseButton::Left),
                MotionAction::HoverEnter | MotionAction::HoverMove | MotionAction::HoverExit => {
                    MouseEventKind::Moved
                }
                // For any other action, ignore
                _ => return None,
            };
            
            Some(CrosstermEvent::Mouse(MouseEvent {
                kind,
                column: col,
                row,
                modifiers: KeyModifiers::empty(),
            }))
        }
        _ => None,
    }
}

/// Convert keyboard key name to crossterm event
pub fn keyboard_key_to_event(key_name: &str) -> Option<CrosstermEvent> {
    let key_code = match key_name {
        "ESC" => KeyCode::Esc,
        "TAB" => KeyCode::Tab,
        "SHIFT" | "CTRL" | "KEYBOARD" => return None,
        "UP" => KeyCode::Up,
        "DOWN" => KeyCode::Down,
        "LEFT" => KeyCode::Left,
        "RIGHT" => KeyCode::Right,
        "ENTER" => KeyCode::Enter,
        "SPACE" => KeyCode::Char(' '),
        "BACKSPACE" => KeyCode::Backspace,
        "DELETE" => KeyCode::Delete,
        _ => return None,
    };
    Some(CrosstermEvent::Key(CrosstermKeyEvent::new(key_code, KeyModifiers::empty())))
}

/// Validate input coordinates
pub fn validate_coordinates(x: f32, y: f32, max_x: f32, max_y: f32) -> bool {
    x >= 0.0 && y >= 0.0 && x <= max_x && y <= max_y
}

