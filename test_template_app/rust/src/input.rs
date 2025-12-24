//! Input adapter that converts Android input events to Ratatui/Crossterm events.
//! This allows TUI widgets to react to touch and keyboard input naturally.

#[cfg(target_os = "android")]
use android_activity::input::{InputEvent, MotionAction, KeyAction, Keycode};
#[cfg(target_os = "android")]
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Converts raw Android Input to a Ratatui-compatible Event
pub fn map_android_event(event: &InputEvent, font_width: f32, font_height: f32) -> Option<Event> {
    match event {
        InputEvent::KeyEvent(key) => {
            if key.action() == KeyAction::Down {
                map_key_code(key.key_code()).map(Event::Key)
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

fn map_key_code(code: Keycode) -> Option<KeyEvent> {
    // Basic mapping - expand this for full keyboard support
    let code = match code {
        // Letters (lowercase for now - Android keyboard handles case)
        Keycode::A => KeyCode::Char('a'),
        Keycode::B => KeyCode::Char('b'),
        Keycode::C => KeyCode::Char('c'),
        Keycode::D => KeyCode::Char('d'),
        Keycode::E => KeyCode::Char('e'),
        Keycode::F => KeyCode::Char('f'),
        Keycode::G => KeyCode::Char('g'),
        Keycode::H => KeyCode::Char('h'),
        Keycode::I => KeyCode::Char('i'),
        Keycode::J => KeyCode::Char('j'),
        Keycode::K => KeyCode::Char('k'),
        Keycode::L => KeyCode::Char('l'),
        Keycode::M => KeyCode::Char('m'),
        Keycode::N => KeyCode::Char('n'),
        Keycode::O => KeyCode::Char('o'),
        Keycode::P => KeyCode::Char('p'),
        Keycode::Q => KeyCode::Char('q'),
        Keycode::R => KeyCode::Char('r'),
        Keycode::S => KeyCode::Char('s'),
        Keycode::T => KeyCode::Char('t'),
        Keycode::U => KeyCode::Char('u'),
        Keycode::V => KeyCode::Char('v'),
        Keycode::W => KeyCode::Char('w'),
        Keycode::X => KeyCode::Char('x'),
        Keycode::Y => KeyCode::Char('y'),
        Keycode::Z => KeyCode::Char('z'),
        // Special keys
        Keycode::Enter => KeyCode::Enter,
        Keycode::Space => KeyCode::Char(' '),
        Keycode::Del => KeyCode::Backspace,
        Keycode::Escape | Keycode::Back => KeyCode::Esc,
        Keycode::DpadUp => KeyCode::Up,
        Keycode::DpadDown => KeyCode::Down,
        Keycode::DpadLeft => KeyCode::Left,
        Keycode::DpadRight => KeyCode::Right,
        _ => return None,
    };

    Some(KeyEvent::new(code, KeyModifiers::empty()))
}

