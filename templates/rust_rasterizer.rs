//! Software rasterizer that converts Ratatui cells to pixels.
//! This acts as the "GPU" for our terminal emulator, rendering
//! characters from a font file into a pixel buffer.

use ab_glyph::{Font, FontRef};
use ab_glyph::scale::Scale;
use ratatui::style::Color;

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
        let scale = Scale::uniform(size);
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
    /// stride: The width of the Android window in pixels
    pub fn render_to_surface(
        &self, 
        backend: &super::backend::AndroidBackend, 
        dest: &mut [u8], 
        stride: usize,
        window_height: usize,
    ) {
        // Clear screen (black background)
        dest.fill(0);

        // Iterate over every cell in the backend
        for y in 0..backend.height {
            for x in 0..backend.width {
                if let Some(cell) = backend.get_cell(x, y) {
                    let ch = cell.symbol().chars().next().unwrap_or(' ');
                    let fg_color = self.color_to_rgba(cell.fg);
                    let bg_color = self.color_to_rgba(cell.bg);
                    
                    // Calculate pixel position
                    let px = (x as f32 * self.font_width) as usize;
                    let py = (y as f32 * self.font_height) as usize;
                    
                    // Draw background
                    self.draw_rect(px, py, self.font_width as usize, self.font_height as usize, bg_color, dest, stride, window_height);
                    
                    // Draw character
                    self.draw_char(ch, px, py, fg_color, dest, stride, window_height);
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
        stride: usize,
        window_height: usize,
    ) {
        for dy in 0..h {
            let py = y + dy;
            if py >= window_height {
                break;
            }
            for dx in 0..w {
                let px = x + dx;
                if px >= stride {
                    break;
                }
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

    fn draw_char(
        &self,
        c: char,
        px: usize,
        py: usize,
        color: [u8; 4],
        dest: &mut [u8],
        stride: usize,
        window_height: usize,
    ) {
        // Try to render with font, fallback to simple block rendering if font fails
        let scale = Scale::uniform(self.font_size);
        let glyph = self.font.glyph_id(c).with_scale(scale);
        
        // Try to render with font
        if let Some(outlined) = self.font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let offset_x = bounds.min.x;
            let offset_y = bounds.min.y;
            
            outlined.draw(|x, y, coverage| {
                let global_x = (px as f32 + offset_x + x as f32) as usize;
                let global_y = (py as f32 + offset_y + y as f32) as usize;
                
                if global_y >= window_height || global_x >= stride {
                    return;
                }
                
                let idx = (global_y * stride + global_x) * 4;
                if idx + 3 < dest.len() {
                    let alpha = (coverage * color[3] as f32) as u8;
                    // Alpha blend with background
                    let bg_alpha = 255u16.saturating_sub(alpha as u16);
                    dest[idx] = ((dest[idx] as u16 * bg_alpha / 255) + (color[0] as u16 * alpha as u16 / 255)) as u8;
                    dest[idx + 1] = ((dest[idx + 1] as u16 * bg_alpha / 255) + (color[1] as u16 * alpha as u16 / 255)) as u8;
                    dest[idx + 2] = ((dest[idx + 2] as u16 * bg_alpha / 255) + (color[2] as u16 * alpha as u16 / 255)) as u8;
                    dest[idx + 3] = dest[idx + 3].max(alpha);
                }
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
                if x >= stride {
                    break;
                }
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
            Color::Reset => [255, 255, 255, 255], // White
            Color::Black => [0, 0, 0, 255],
            Color::Red => [255, 0, 0, 255],
            Color::Green => [0, 255, 0, 255],
            Color::Yellow => [255, 255, 0, 255],
            Color::Blue => [0, 0, 255, 255],
            Color::Magenta => [255, 0, 255, 255],
            Color::Cyan => [0, 255, 255, 255],
            Color::White => [255, 255, 255, 255],
            Color::Gray => [128, 128, 128, 255],
            Color::DarkGray => [64, 64, 64, 255],
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

