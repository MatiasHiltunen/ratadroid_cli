//! Direct keyboard renderer that bypasses TUI/cosmic-text.
//!
//! Renders keyboard buttons directly to the pixel buffer for reliability
//! and consistent positioning at the bottom of the screen.
//!
//! This is preferred over [`KeyboardWidget`] when you need precise pixel-level
//! control over the keyboard position, especially when dealing with system
//! UI elements like navigation bars.

use std::time::Instant;

/// State for the direct keyboard.
#[derive(Default)]
pub struct DirectKeyboardState {
    /// Currently pressed key (for visual feedback)
    pub pressed_key: Option<String>,
    /// When the key was pressed (for visual feedback timeout)
    pub press_time: Option<Instant>,
    /// Whether Shift modifier is active (toggle)
    pub shift_active: bool,
    /// Whether Ctrl modifier is active (toggle)
    pub ctrl_active: bool,
}

impl DirectKeyboardState {
    /// Create a new keyboard state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a key as pressed (for visual feedback).
    pub fn set_pressed(&mut self, key: String) {
        self.pressed_key = Some(key);
        self.press_time = Some(Instant::now());
    }

    /// Clear the pressed key state.
    pub fn clear_pressed(&mut self) {
        self.pressed_key = None;
        self.press_time = None;
    }

    /// Toggle Shift modifier.
    pub fn toggle_shift(&mut self) {
        self.shift_active = !self.shift_active;
    }

    /// Toggle Ctrl modifier.
    pub fn toggle_ctrl(&mut self) {
        self.ctrl_active = !self.ctrl_active;
    }

    /// Check if visual feedback should still be shown.
    pub fn should_show_feedback(&self) -> bool {
        if let Some(time) = self.press_time {
            time.elapsed().as_millis() < 200
        } else {
            false
        }
    }
}

/// Button definition.
struct Button {
    label: &'static str,
    key_name: &'static str,
    width_units: u32,
}

/// Direct keyboard renderer.
///
/// Renders a keyboard directly to a pixel buffer, bypassing Ratatui's
/// cell-based rendering for precise control.
pub struct DirectKeyboard {
    row1: Vec<Button>,
    row2: Vec<Button>,
    row2_padding: u32,
}

impl Default for DirectKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectKeyboard {
    /// Create a new direct keyboard with default layout.
    pub fn new() -> Self {
        Self {
            row1: vec![
                Button { label: "ESC", key_name: "ESC", width_units: 1 },
                Button { label: "TAB", key_name: "TAB", width_units: 1 },
                Button { label: "SFT", key_name: "SHIFT", width_units: 1 },
                Button { label: "CTL", key_name: "CTRL", width_units: 1 },
                Button { label: "^", key_name: "UP", width_units: 1 },
                Button { label: "DEL", key_name: "DELETE", width_units: 1 },
                Button { label: "RET", key_name: "ENTER", width_units: 1 },
            ],
            row2: vec![
                Button { label: "<", key_name: "LEFT", width_units: 1 },
                Button { label: "v", key_name: "DOWN", width_units: 1 },
                Button { label: ">", key_name: "RIGHT", width_units: 1 },
                Button { label: "KB", key_name: "KEYBOARD", width_units: 1 },
            ],
            row2_padding: 3,
        }
    }

    /// Get the keyboard height in pixels based on button height.
    pub fn height_pixels(&self, button_height: u32) -> u32 {
        button_height * 2 + 4
    }

    /// Render the keyboard directly to a pixel buffer.
    ///
    /// # Arguments
    ///
    /// * `state` - Keyboard state for visual feedback
    /// * `dest` - Destination pixel buffer (RGBA format)
    /// * `stride` - Buffer stride (pixels per row)
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    /// * `keyboard_y` - Y position where keyboard starts
    /// * `button_height` - Height of each button row in pixels
    pub fn render(
        &self,
        state: &DirectKeyboardState,
        dest: &mut [u8],
        stride: usize,
        window_width: usize,
        _window_height: usize,
        keyboard_y: usize,
        button_height: u32,
    ) {
        let button_height = button_height as usize;
        let total_units: u32 = self.row1.iter().map(|b| b.width_units).sum();
        // Increased max button width from 100 to 150 for better touch targets
        let button_width = (window_width / total_units as usize).min(150);
        let keyboard_width = button_width * total_units as usize;
        let keyboard_x = (window_width.saturating_sub(keyboard_width)) / 2;

        // Colors
        let bg_dark: [u8; 4] = [40, 40, 40, 255];
        let bg_active: [u8; 4] = [0, 200, 100, 255];
        let bg_toggle: [u8; 4] = [0, 150, 200, 255];
        let bg_blue: [u8; 4] = [50, 80, 200, 255];
        let fg_white: [u8; 4] = [255, 255, 255, 255];
        let fg_yellow: [u8; 4] = [255, 220, 100, 255];
        let border: [u8; 4] = [80, 80, 80, 255];
        let kb_bg: [u8; 4] = [20, 20, 20, 255];

        // Draw keyboard background
        let max_y = (keyboard_y + button_height * 2 + 4).min(dest.len() / (stride * 4));
        for y in keyboard_y..max_y {
            for x in 0..window_width {
                let idx = (y * stride + x) * 4;
                if idx + 3 < dest.len() {
                    dest[idx] = kb_bg[0];
                    dest[idx + 1] = kb_bg[1];
                    dest[idx + 2] = kb_bg[2];
                    dest[idx + 3] = kb_bg[3];
                }
            }
        }

        // Row 1
        let row1_y = keyboard_y + 1;
        let mut x_offset = keyboard_x;
        for button in &self.row1 {
            let btn_width = button_width * button.width_units as usize;
            let is_pressed = state.pressed_key.as_deref() == Some(button.key_name);
            let is_shift_toggle = button.key_name == "SHIFT" && state.shift_active;
            let is_ctrl_toggle = button.key_name == "CTRL" && state.ctrl_active;
            let is_special = button.key_name == "ENTER";
            let is_arrow = button.key_name == "UP";

            let bg = if is_pressed {
                bg_active
            } else if is_shift_toggle || is_ctrl_toggle {
                bg_toggle
            } else if is_special {
                bg_blue
            } else {
                bg_dark
            };

            let fg = if is_arrow { fg_yellow } else { fg_white };

            self.draw_button(dest, stride, x_offset, row1_y, btn_width, button_height, button.label, bg, fg, border);
            x_offset += btn_width + 2;
        }

        // Row 2
        let row2_y = keyboard_y + button_height + 3;
        x_offset = keyboard_x + (button_width * self.row2_padding as usize) + (2 * self.row2_padding as usize);
        for button in &self.row2 {
            let btn_width = button_width * button.width_units as usize;
            let is_pressed = state.pressed_key.as_deref() == Some(button.key_name);
            let is_special = button.key_name == "KEYBOARD";
            let is_arrow = matches!(button.key_name, "LEFT" | "DOWN" | "RIGHT");

            let bg = if is_pressed {
                bg_active
            } else if is_special {
                bg_blue
            } else {
                bg_dark
            };

            let fg = if is_arrow { fg_yellow } else { fg_white };

            self.draw_button(dest, stride, x_offset, row2_y, btn_width, button_height, button.label, bg, fg, border);
            x_offset += btn_width + 2;
        }
    }

    fn draw_button(
        &self,
        dest: &mut [u8],
        stride: usize,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        label: &str,
        bg: [u8; 4],
        fg: [u8; 4],
        border: [u8; 4],
    ) {
        for py in y..(y + height) {
            for px in x..(x + width) {
                let idx = (py * stride + px) * 4;
                if idx + 3 >= dest.len() {
                    continue;
                }

                let is_corner = (py == y || py == y + height - 1) && (px == x || px == x + width - 1);
                let is_border = py == y || py == y + height - 1 || px == x || px == x + width - 1;

                let color = if is_corner {
                    [20, 20, 20, 255]
                } else if is_border {
                    border
                } else {
                    bg
                };

                dest[idx] = color[0];
                dest[idx + 1] = color[1];
                dest[idx + 2] = color[2];
                dest[idx + 3] = color[3];
            }
        }

        self.draw_text(dest, stride, x, y, width, height, label, fg);
    }

    fn draw_text(
        &self,
        dest: &mut [u8],
        stride: usize,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        text: &str,
        color: [u8; 4],
    ) {
        let char_width = 5;
        let char_height = 7;
        let char_spacing = 1;

        let total_text_width = text.len() * (char_width + char_spacing) - char_spacing;
        let start_x = x + (width.saturating_sub(total_text_width)) / 2;
        let start_y = y + (height.saturating_sub(char_height)) / 2;

        for (i, ch) in text.chars().enumerate() {
            let char_x = start_x + i * (char_width + char_spacing);
            self.draw_char(dest, stride, char_x, start_y, ch, color);
        }
    }

    fn draw_char(&self, dest: &mut [u8], stride: usize, x: usize, y: usize, ch: char, color: [u8; 4]) {
        let pattern: [u8; 7] = match ch {
            'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
            'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
            'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
            'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
            'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
            'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
            'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
            'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
            'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
            'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
            'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
            'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
            '^' => [0b00100, 0b01110, 0b10101, 0b00100, 0b00100, 0b00100, 0b00000],
            'v' => [0b00000, 0b00100, 0b00100, 0b00100, 0b10101, 0b01110, 0b00100],
            '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
            '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
            _ => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        };

        for (row, &bits) in pattern.iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 1 {
                    let px = x + col;
                    let py = y + row;
                    let idx = (py * stride + px) * 4;
                    if idx + 3 < dest.len() {
                        dest[idx] = color[0];
                        dest[idx + 1] = color[1];
                        dest[idx + 2] = color[2];
                        dest[idx + 3] = color[3];
                    }
                }
            }
        }
    }

    /// Handle touch at the given pixel coordinates.
    /// Returns `Some(key_name)` if a button was pressed.
    pub fn handle_touch(
        &self,
        touch_x: usize,
        touch_y: usize,
        window_width: usize,
        keyboard_y: usize,
        button_height: u32,
    ) -> Option<&'static str> {
        let button_height = button_height as usize;
        let total_units: u32 = self.row1.iter().map(|b| b.width_units).sum();
        // Must match the render() button width calculation
        let button_width = (window_width / total_units as usize).min(150);
        let keyboard_width = button_width * total_units as usize;
        let keyboard_x = (window_width.saturating_sub(keyboard_width)) / 2;

        // Row 1
        let row1_y = keyboard_y + 1;
        if touch_y >= row1_y && touch_y < row1_y + button_height {
            let mut x_offset = keyboard_x;
            for button in &self.row1 {
                let btn_width = button_width * button.width_units as usize;
                if touch_x >= x_offset && touch_x < x_offset + btn_width {
                    return Some(button.key_name);
                }
                x_offset += btn_width + 2;
            }
        }

        // Row 2
        let row2_y = keyboard_y + button_height + 3;
        if touch_y >= row2_y && touch_y < row2_y + button_height {
            let mut x_offset = keyboard_x + (button_width * self.row2_padding as usize) + (2 * self.row2_padding as usize);
            for button in &self.row2 {
                let btn_width = button_width * button.width_units as usize;
                if touch_x >= x_offset && touch_x < x_offset + btn_width {
                    return Some(button.key_name);
                }
                x_offset += btn_width + 2;
            }
        }

        None
    }
}

