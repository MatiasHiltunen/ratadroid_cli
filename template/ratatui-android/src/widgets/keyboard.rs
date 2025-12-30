//! On-screen keyboard widget for TUI applications.
//!
//! Provides a Ratatui widget for rendering an on-screen keyboard with special keys
//! commonly needed in TUI applications.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ratatui_android::widgets::{KeyboardWidget, KeyboardState};
//! use ratatui::Frame;
//!
//! let mut state = KeyboardState::new();
//!
//! // In your draw function:
//! frame.render_widget(KeyboardWidget::new(&mut state), keyboard_area);
//! ```

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget},
};
use std::time::Instant;

/// State for the on-screen keyboard widget.
///
/// Tracks pressed keys for visual feedback and modifier states (Shift, Ctrl).
pub struct KeyboardState {
    /// Currently pressed key (for visual feedback)
    pub pressed_key: Option<String>,
    /// When the key was pressed (for visual feedback timeout)
    pub press_time: Option<Instant>,
    /// Whether Shift modifier is active (toggle)
    pub shift_active: bool,
    /// Whether Ctrl modifier is active (toggle)
    pub ctrl_active: bool,
    /// Starting column of row 1 (set during rendering for touch detection)
    pub row1_start_col: Option<u16>,
    /// Starting column of row 2 (set during rendering for touch detection)
    pub row2_start_col: Option<u16>,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            pressed_key: None,
            press_time: None,
            shift_active: false,
            ctrl_active: false,
            row1_start_col: None,
            row2_start_col: None,
        }
    }
}

impl KeyboardState {
    /// Create a new keyboard state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the pressed key state (call after handling the key press).
    pub fn clear_pressed(&mut self) {
        self.pressed_key = None;
        self.press_time = None;
    }

    /// Set a key as pressed (for visual feedback).
    pub fn set_pressed(&mut self, key: String) {
        self.pressed_key = Some(key);
        self.press_time = Some(Instant::now());
    }

    /// Toggle Shift modifier.
    pub fn toggle_shift(&mut self) {
        self.shift_active = !self.shift_active;
    }

    /// Toggle Ctrl modifier.
    pub fn toggle_ctrl(&mut self) {
        self.ctrl_active = !self.ctrl_active;
    }

    /// Check if visual feedback should still be shown (within timeout).
    pub fn should_show_feedback(&self) -> bool {
        if let Some(time) = self.press_time {
            time.elapsed().as_millis() < 200
        } else {
            false
        }
    }
}

/// On-screen keyboard widget.
///
/// Renders a two-row keyboard with special keys:
/// - Row 1: ESC, Tab, Shift, Ctrl, Up, Delete, Enter
/// - Row 2: (padding), Left, Down, Right, Keyboard (toggle)
pub struct KeyboardWidget<'a> {
    state: &'a mut KeyboardState,
}

impl<'a> KeyboardWidget<'a> {
    /// Create a new keyboard widget with the given state.
    pub fn new(state: &'a mut KeyboardState) -> Self {
        Self { state }
    }

    /// Handle a touch event at the given coordinates.
    /// Returns `Some(key_name)` if a key was pressed, `None` otherwise.
    pub fn handle_touch(&mut self, col: u16, row: u16, keyboard_start_row: u16) -> Option<String> {
        let keyboard_row = row.saturating_sub(keyboard_start_row);

        if keyboard_row == 0 {
            // Row 1: ESC(3) Tab(3) Shift(3) Ctrl(3) Up(3) Delete(3) Enter(3) = 21 cols
            let keyboard_width = 21u16;
            let keyboard_start_col = self.state.row1_start_col?;
            let keyboard_end_col = keyboard_start_col + keyboard_width;

            if col < keyboard_start_col || col >= keyboard_end_col {
                return None;
            }

            let relative_col = col - keyboard_start_col;

            let key_name = if relative_col < 3 {
                "ESC"
            } else if relative_col < 6 {
                "TAB"
            } else if relative_col < 9 {
                "SHIFT"
            } else if relative_col < 12 {
                "CTRL"
            } else if relative_col < 15 {
                "UP"
            } else if relative_col < 18 {
                "DELETE"
            } else {
                "ENTER"
            };

            self.state.set_pressed(key_name.to_string());
            return Some(key_name.to_string());
        } else if keyboard_row == 1 {
            let keyboard_width = 21u16;
            let keyboard_start_col = self.state.row2_start_col?;
            let keyboard_end_col = keyboard_start_col + keyboard_width;

            if col < keyboard_start_col || col >= keyboard_end_col {
                return None;
            }

            let relative_col = col - keyboard_start_col;

            let key_name = if relative_col >= 9 && relative_col < 12 {
                Some("LEFT")
            } else if relative_col >= 12 && relative_col < 15 {
                Some("DOWN")
            } else if relative_col >= 15 && relative_col < 18 {
                Some("RIGHT")
            } else if relative_col >= 18 && relative_col < 21 {
                Some("KEYBOARD")
            } else {
                None
            };

            if let Some(key) = key_name {
                self.state.set_pressed(key.to_string());
                return Some(key.to_string());
            }
        }

        None
    }
}

impl<'a> Widget for KeyboardWidget<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.height < 2 {
            return;
        }

        let keyboard_width = 21u16;
        let keyboard_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        let keyboard_start_col = (area.width.saturating_sub(keyboard_width)) / 2;

        // Row 1
        let keyboard_centered_row1 = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(keyboard_start_col),
                Constraint::Length(keyboard_width),
                Constraint::Min(0),
            ])
            .split(keyboard_chunks[0]);

        self.state.row1_start_col = Some(area.x + keyboard_centered_row1[1].x);

        let row1_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(3), // ESC
                Constraint::Length(3), // Tab
                Constraint::Length(3), // Shift
                Constraint::Length(3), // Ctrl
                Constraint::Length(3), // Up
                Constraint::Length(3), // Delete
                Constraint::Length(3), // Enter
            ])
            .split(keyboard_centered_row1[1]);

        if row1_chunks.len() >= 7 {
            render_key("␛", "ESC", self.state, row1_chunks[0], buf, false, false);
            render_key("⇥", "TAB", self.state, row1_chunks[1], buf, false, false);
            render_key("⇧", "SHIFT", self.state, row1_chunks[2], buf, self.state.shift_active, false);
            render_key("⌃", "CTRL", self.state, row1_chunks[3], buf, self.state.ctrl_active, false);
            render_key("↑", "UP", self.state, row1_chunks[4], buf, false, true);
            render_key("⌦", "DELETE", self.state, row1_chunks[5], buf, false, false);
            render_key("⏎", "ENTER", self.state, row1_chunks[6], buf, false, false);
        }

        // Row 2
        let keyboard_centered_row2 = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(keyboard_start_col),
                Constraint::Length(keyboard_width),
                Constraint::Min(0),
            ])
            .split(keyboard_chunks[1]);

        self.state.row2_start_col = Some(area.x + keyboard_centered_row2[1].x);

        let row2_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(9), // Padding
                Constraint::Length(3), // Left
                Constraint::Length(3), // Down
                Constraint::Length(3), // Right
                Constraint::Length(3), // Keyboard
            ])
            .split(keyboard_centered_row2[1]);

        if row2_chunks.len() >= 5 {
            render_key("←", "LEFT", self.state, row2_chunks[1], buf, false, true);
            render_key("↓", "DOWN", self.state, row2_chunks[2], buf, false, true);
            render_key("→", "RIGHT", self.state, row2_chunks[3], buf, false, true);
            render_key("⌨", "KEYBOARD", self.state, row2_chunks[4], buf, false, false);
        }
    }
}

fn render_key(
    label: &str,
    key_name: &str,
    state: &KeyboardState,
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    is_toggled: bool,
    is_arrow: bool,
) {
    let is_pressed = state.pressed_key.as_deref() == Some(key_name);
    let style = if is_pressed {
        Style::default().fg(Color::Black).bg(Color::Green)
    } else if is_toggled {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else if is_arrow {
        Style::default().fg(Color::Yellow).bg(Color::DarkGray)
    } else if key_name == "ENTER" || key_name == "KEYBOARD" {
        Style::default().fg(Color::White).bg(Color::Blue)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };

    Paragraph::new(label)
        .style(style)
        .alignment(Alignment::Center)
        .render(area, buf);
}

