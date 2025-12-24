//! Software rasterizer that converts Ratatui cells to pixels.
//! This acts as the "GPU" for our terminal emulator, rendering
//! characters from a font file into a pixel buffer.

use ab_glyph::{Font, FontRef, PxScale};
use ratatui::style::Color;
use ratatui::buffer::Cell;
use log;

pub struct Rasterizer<'a> {
    font: FontRef<'a>,
    font_width: f32,
    font_height: f32,
    font_size: f32,
}

impl<'a> Rasterizer<'a> {
    pub fn new(font_data: &'a [u8], size: f32) -> Result<Self, String> {
        if font_data.is_empty() || font_data.len() < 100 {
            return Err("Font data is empty or too small".to_string());
        }
        
        let font = FontRef::try_from_slice(font_data)
            .map_err(|e| format!("Failed to parse font: {:?}", e))?;
        
        // Measure a typical character to determine cell size
        let scale = PxScale::from(size);
        let glyph = font.glyph_id('M').with_scale(scale);
        
        let (width, height) = if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            (bounds.width().max(size * 0.6), bounds.height().max(size))
        } else {
            (size * 0.6, size)
        };
        
        Ok(Self {
            font,
            font_width: width,
            font_height: height,
            font_size: size,
        })
    }

    /// Create a fallback rasterizer when font loading fails
    /// This uses simple block rendering instead of font glyphs
    pub fn new_fallback(size: f32) -> Self {
        // Create a dummy font reference - we won't use it, but need it for the struct
        // We'll detect font loading failure in draw_char and use block rendering
        let dummy_font_data = b"dummy";
        let font = FontRef::try_from_slice(dummy_font_data).unwrap_or_else(|_| {
            // If even dummy fails, create a zeroed font (we'll handle this in draw_char)
            unsafe { std::mem::zeroed() }
        });
        
        Self {
            font,
            font_width: size * 0.6,
            font_height: size,
            font_size: size,
        }
    }

    /// Renders the backend's cell buffer to a pixel surface
    /// dest: Mutable slice of RGBA pixels (u8)
    /// stride: Buffer stride in pixels (for calculating byte offsets)
    /// window_width: Window width in pixels (for bounds checking)
    /// window_height: Window height in pixels
    pub fn render_to_surface(
        &self, 
        backend: &super::backend::AndroidBackend, 
        dest: &mut [u8], 
        stride: usize,
        window_width: usize,
        window_height: usize,
    ) {
        self.render_to_surface_with_offset(backend, dest, stride, window_width, window_height, 0);
    }

    /// Render with vertical offset (for status bar)
    /// top_offset_px: Number of pixels to skip at the top
    pub fn render_to_surface_with_offset(
        &self, 
        backend: &super::backend::AndroidBackend, 
        dest: &mut [u8], 
        stride: usize,
        window_width: usize,
        window_height: usize,
        top_offset_px: usize,
    ) {
        // Safety check: ensure we have enough buffer space
        // Buffer size is stride * height * 4 (stride includes padding)
        let expected_buffer_size = stride * window_height * 4;
        if dest.len() < expected_buffer_size {
            log::warn!("Buffer too small! dest.len()={}, expected={}", dest.len(), expected_buffer_size);
            return;
        }

        // ROW-BY-ROW RENDERING APPROACH
        // Render entire rows at once for safer sequential memory access
        let font_w = self.font_width as usize;
        let font_h = self.font_height as usize;
        
        // Iterate row by row (by terminal row, not pixel row)
        for term_y in 0..backend.height {
            let py_start = (term_y as f32 * self.font_height) as usize + top_offset_px;
            
            // Skip if this row would be out of bounds
            if py_start >= window_height {
                continue;
            }
            
            // Calculate how many pixel rows this terminal row spans
            let py_end = ((term_y + 1) as f32 * self.font_height) as usize;
            let row_height = (py_end - py_start).min(window_height - py_start);
            
            if row_height == 0 {
                continue;
            }
            
            // Render all cells in this terminal row
            for term_x in 0..backend.width {
                if let Some(cell) = backend.get_cell(term_x, term_y) {
                    let px_start = (term_x as f32 * self.font_width) as usize;
                    
                    // Skip if this cell would be out of bounds
                    if px_start >= window_width {
                        continue;
                    }
                    
                    // Calculate how many pixel columns this terminal cell spans
                    let px_end = ((term_x + 1) as f32 * self.font_width) as usize;
                    let cell_width = (px_end - px_start).min(window_width - px_start);
                    
                    if cell_width == 0 {
                        continue;
                    }
                    
                    let ch = cell.symbol().chars().next().unwrap_or(' ');
                    let fg_color = self.color_to_rgba(cell.fg);
                    let bg_color = self.color_to_rgba(cell.bg);
                    
                    // Render this cell row-by-row within the cell bounds
                    for py_offset in 0..row_height {
                        let py = py_start + py_offset;
                        if py >= window_height {
                            break;
                        }
                        
                        // Calculate row start index in buffer
                        let row_start_idx = match py.checked_mul(stride)
                            .and_then(|row_offset| row_offset.checked_add(px_start))
                            .and_then(|pixel_offset| pixel_offset.checked_mul(4))
                        {
                            Some(idx) => idx,
                            None => continue, // Overflow, skip this row
                        };
                        
                        // Ensure we have enough space for this row using checked arithmetic
                        let row_end_idx = match cell_width.checked_mul(4)
                            .and_then(|bytes| row_start_idx.checked_add(bytes))
                        {
                            Some(idx) => idx,
                            None => break, // Overflow, skip this row
                        };
                        if row_end_idx > dest.len() {
                            break; // Out of bounds, stop rendering this row
                        }
                        
                        // Render this pixel row of the cell
                        for px_offset in 0..cell_width {
                            let px = px_start + px_offset;
                            if px >= window_width {
                                break;
                            }
                            
                            // Use checked arithmetic for index calculation
                            let idx = match px_offset.checked_mul(4)
                                .and_then(|bytes| row_start_idx.checked_add(bytes))
                            {
                                Some(idx) => idx,
                                None => break, // Overflow, skip remaining pixels
                            };
                            
                            if idx.saturating_add(3) >= dest.len() {
                                break;
                            }
                            
                            // Write background color (we'll overlay character later)
                            dest[idx] = bg_color[0];
                            dest[idx + 1] = bg_color[1];
                            dest[idx + 2] = bg_color[2];
                            dest[idx + 3] = bg_color[3];
                        }
                    }
                    
                    // Now draw the character on top (if not a space)
                    // Adjust vertical position to account for font baseline
                    // Font glyphs have negative min.y offsets (they extend above baseline)
                    // We need to shift the character down to align with the cell
                    if ch != ' ' {
                        // Calculate baseline offset from font metrics
                        // Most fonts have min.y around -font_height * 0.75 to -font_height * 0.85
                        // This represents how far above the baseline the tallest character extends
                        let scale = PxScale::from(self.font_size);
                        let glyph = self.font.glyph_id('M').with_scale(scale); // Use 'M' as reference
                        let baseline_offset = if let Some(outlined) = self.font.outline_glyph(glyph) {
                            let bounds = outlined.px_bounds();
                            // bounds.min.y is negative, so we add its absolute value to shift down
                            // This aligns the character's visual top with the cell top
                            (-bounds.min.y).max(0.0) as usize
                        } else {
                            // Fallback: estimate baseline offset as ~20% of font height
                            (self.font_height * 0.2) as usize
                        };
                        // Draw character aligned to cell top + baseline offset
                        // This ensures the character appears at the correct vertical position
                        let char_y = py_start.saturating_add(baseline_offset);
                        self.draw_char(ch, px_start, char_y, fg_color, dest, stride, window_width, window_height);
                    }
                }
            }
        }
    }

    fn draw_rect(
        &self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: [u8; 4],
        dest: &mut [u8],
        stride: usize,        // Buffer stride for calculating byte offsets
        window_width: usize,   // Window width for bounds checking
        window_height: usize,
    ) {
        // Early return if parameters are invalid
        if x >= window_width || y >= window_height || w == 0 || h == 0 {
            return;
        }
        
        // Clamp width and height to fit within bounds
        let w_clamped = w.min(window_width.saturating_sub(x));
        let h_clamped = h.min(window_height.saturating_sub(y));
        
        // Early return if clamped dimensions are zero
        if w_clamped == 0 || h_clamped == 0 {
            return;
        }
        
        // Pre-calculate maximum safe index to avoid repeated calculations
        let max_safe_idx = dest.len().saturating_sub(4);
        
        for dy in 0..h_clamped {
            let py = y + dy;
            // Additional bounds check for y coordinate
            if py >= window_height {
                break;
            }
            for dx in 0..w_clamped {
                let px = x + dx;
                // Additional bounds check for x coordinate
                if px >= window_width {
                    break;
                }
                // Use stride for buffer layout (stride can be larger than window_width)
                // Use checked arithmetic to prevent overflow
                let idx = match py.checked_mul(stride)
                    .and_then(|row_offset| row_offset.checked_add(px))
                    .and_then(|pixel_offset| pixel_offset.checked_mul(4))
                {
                    Some(idx) => idx,
                    None => {
                        // Overflow in index calculation - skip this pixel
                        continue;
                    }
                };
                // Bounds check: ensure we have at least 4 bytes (RGBA) remaining
                if idx > max_safe_idx {
                    // Out of bounds - skip this pixel
                    continue;
                }
                // Final safety check before writing
                if idx + 3 >= dest.len() {
                    continue;
                }
                // Safe to write - use unchecked indexing since we've validated
                unsafe {
                    *dest.get_unchecked_mut(idx) = color[0];
                    *dest.get_unchecked_mut(idx + 1) = color[1];
                    *dest.get_unchecked_mut(idx + 2) = color[2];
                    *dest.get_unchecked_mut(idx + 3) = color[3];
                }
            }
        }
    }

    fn draw_char(
        &self,
        c: char,
        px: usize,
        py: usize,
        color: [u8; 4],
        dest: &mut [u8],
        stride: usize,        // Buffer stride for calculating byte offsets
        window_width: usize,   // Window width for bounds checking
        window_height: usize,
    ) {
        // Try to render with font, fallback to simple block rendering if font fails
        let scale = PxScale::from(self.font_size);
        let glyph = self.font.glyph_id(c).with_scale(scale);
        
        // Try to render with font
        if let Some(outlined) = self.font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let offset_x = bounds.min.x;
            let offset_y = bounds.min.y;
            
            // Pre-check: ensure the glyph won't extend beyond bounds
            // Calculate maximum extent of the glyph - handle negative offsets safely
            let max_x = (px as f32 + offset_x + bounds.width()) as usize;
            let max_y = (py as f32 + offset_y + bounds.height()) as usize;
            
            // If glyph extends beyond bounds, skip it
            if max_x > window_width || max_y > window_height || px >= window_width || py >= window_height {
                // Glyph would extend beyond bounds - skip rendering
                return;
            }
            
            // Additional safety: ensure offset calculations won't cause underflow
            let px_f32 = px as f32;
            let py_f32 = py as f32;
            
            outlined.draw(|x, y, coverage| {
                // Calculate global coordinates without clamping first
                let global_x_f32 = px_f32 + offset_x + x as f32;
                let global_y_f32 = py_f32 + offset_y + y as f32;
                
                // Bounds check BEFORE converting to usize - CRITICAL
                // Skip pixels that are outside the window bounds
                if global_x_f32 < 0.0 || global_x_f32 >= window_width as f32 ||
                   global_y_f32 < 0.0 || global_y_f32 >= window_height as f32 {
                    return;
                }
                
                // Now safe to convert to usize (we know they're in bounds)
                let global_x = global_x_f32 as usize;
                let global_y = global_y_f32 as usize;
                
                // Use stride for buffer layout
                // Calculate index: (row * stride + col) * bytes_per_pixel
                // Use checked arithmetic to prevent overflow
                let idx = match (global_y.checked_mul(stride))
                    .and_then(|row_offset| row_offset.checked_add(global_x))
                    .and_then(|pixel_offset| pixel_offset.checked_mul(4))
                {
                    Some(idx) => idx,
                    None => {
                        // Overflow in index calculation - skip this pixel
                        return;
                    }
                };
                
                // Final bounds check for buffer - CRITICAL to prevent crash
                // Ensure we have at least 4 bytes (RGBA) remaining
                if idx.saturating_add(3) >= dest.len() {
                    // Out of bounds - silently skip (shouldn't happen with proper bounds checking above)
                    return;
                }
                
                let alpha = (coverage * color[3] as f32) as u8;
                // Alpha blend with background
                let bg_alpha = 255u16.saturating_sub(alpha as u16);
                dest[idx] = ((dest[idx] as u16 * bg_alpha / 255) + (color[0] as u16 * alpha as u16 / 255)) as u8;
                dest[idx + 1] = ((dest[idx + 1] as u16 * bg_alpha / 255) + (color[1] as u16 * alpha as u16 / 255)) as u8;
                dest[idx + 2] = ((dest[idx + 2] as u16 * bg_alpha / 255) + (color[2] as u16 * alpha as u16 / 255)) as u8;
                dest[idx + 3] = dest[idx + 3].max(alpha);
            });
            return;
        }
        
        // Fallback: draw a simple block for the character
        // This ensures something is visible even if font rendering fails
        let block_size = (self.font_height * 0.8) as usize;
        let start_x = px + (self.font_width as usize - block_size) / 2;
        let start_y = py + (self.font_height as usize - block_size) / 2;
        
        for dy in 0..block_size {
            let y = start_y + dy;
            if y >= window_height {
                break;
            }
            for dx in 0..block_size {
                let x = start_x + dx;
                if x >= window_width {
                    break;
                }
                // Use stride for buffer layout
                let idx = (y * stride + x) * 4;
                if idx + 3 < dest.len() {
                    dest[idx] = color[0];
                    dest[idx + 1] = color[1];
                    dest[idx + 2] = color[2];
                    dest[idx + 3] = color[3];
                }
            }
        }
    }

    fn color_to_rgba(&self, color: Color) -> [u8; 4] {
        match color {
            Color::Reset => [0, 0, 0, 255], // Black
            Color::Black => [0, 0, 0, 255],
            Color::Red => [255, 0, 0, 255],
            Color::Green => [0, 255, 0, 255],
            Color::Yellow => [255, 255, 0, 255],
            Color::Blue => [0, 0, 255, 255],
            Color::Magenta => [255, 0, 255, 255],
            Color::Cyan => [0, 255, 255, 255],
            Color::White => [255, 255, 255, 255],
            Color::Gray => [128, 128, 128, 255],
            Color::DarkGray => [128, 128, 128, 255], // Lighter gray for better visibility
            Color::LightRed => [255, 128, 128, 255],
            Color::LightGreen => [128, 255, 128, 255],
            Color::LightYellow => [255, 255, 128, 255],
            Color::LightBlue => [128, 128, 255, 255],
            Color::LightMagenta => [255, 128, 255, 255],
            Color::LightCyan => [128, 255, 255, 255],
            Color::Rgb(r, g, b) => [r, g, b, 255],
            Color::Indexed(_) => [255, 255, 255, 255], // Default to white for indexed colors
        }
    }

    pub fn font_width(&self) -> f32 {
        self.font_width
    }

    pub fn font_height(&self) -> f32 {
        self.font_height
    }
}

