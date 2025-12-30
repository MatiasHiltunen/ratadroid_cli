//! Rendering logic for Android TUI applications

use log::warn;
use ndk::native_window::NativeWindow;

use super::{AppState, create_context};

/// Height of direct keyboard in pixels (2 button rows + padding)
const DIRECT_KEYBOARD_HEIGHT_PX: u32 = 120;

/// Draw the TUI to the native window
pub fn draw_tui(state: &mut AppState, window: &NativeWindow) {
    // Clear terminal
    if let Err(e) = state.terminal.clear() {
        warn!("Failed to clear terminal: {:?}", e);
        return;
    }
    
    // Draw app UI
    let ctx = create_context(state);
    if let Err(e) = state.terminal.draw(|frame| {
        state.app.draw(frame, &ctx);
    }) {
        warn!("Failed to draw terminal: {:?}", e);
        return;
    }
    
    // Blit to screen
    match window.lock(None) {
        Ok(mut buffer) => {
            let stride = buffer.stride() as usize;
            let height = buffer.height() as usize;
            let bits_ptr = buffer.bits();
            
            if bits_ptr.is_null() {
                warn!("Buffer bits pointer is null");
                return;
            }
            
            let window_width = window.width() as usize;
            let window_height = window.height() as usize;
            let safe_height = height.min(window_height);
            
            if safe_height == 0 {
                warn!("Safe height is 0");
                return;
            }
            
            let max_buffer_size = stride.saturating_mul(safe_height).saturating_mul(4);
            if max_buffer_size == 0 {
                warn!("Max buffer size is 0");
                return;
            }
            
            let pixels_mut = unsafe {
                std::slice::from_raw_parts_mut(bits_ptr as *mut u8, max_buffer_size)
            };
            
            // Clear to black
            for chunk in pixels_mut.chunks_exact_mut(4) {
                chunk[0] = 0; // R
                chunk[1] = 0; // G
                chunk[2] = 0; // B
                chunk[3] = 255; // A
            }
            
            let top_offset_px = (state.top_offset_rows as f32 * state.rasterizer.font_height()) as usize;
            let bottom_offset_px = (state.bottom_offset_rows as f32 * state.rasterizer.font_height()) as usize;
            
            // Render TUI content
            state.rasterizer.render_to_surface_with_offset(
                state.terminal.backend(),
                pixels_mut,
                stride,
                window_width,
                window_height,
                top_offset_px,
                bottom_offset_px,
            );
            
            // Render direct keyboard
            let nav_bar_px = state.nav_bar_height_px as usize;
            let keyboard_y = window_height.saturating_sub(DIRECT_KEYBOARD_HEIGHT_PX as usize + nav_bar_px);
            let button_height = (DIRECT_KEYBOARD_HEIGHT_PX / 2).saturating_sub(4).max(20);
            
            state.direct_keyboard.render(
                &state.direct_keyboard_state,
                pixels_mut,
                stride,
                window_width,
                window_height,
                keyboard_y,
                button_height,
            );
        }
        Err(e) => {
            warn!("Failed to lock buffer: {:?}", e);
        }
    }
}


