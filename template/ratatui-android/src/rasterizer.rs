//! Software rasterizer that converts Ratatui cells to pixels.
//!
//! This acts as the "GPU" for our terminal emulator, rendering characters
//! from fonts into a pixel buffer using cosmic-text for text shaping.
//!
//! ## Features
//!
//! - Proper text shaping with rustybuzz (via cosmic-text)
//! - Font fallback and discovery via fontdb
//! - Emoji and complex Unicode support
//! - Efficient glyph caching via SwashCache and LRU cache
//!
//! ## Usage
//!
//! ```rust
//! use ratatui_android::{AndroidBackend, Rasterizer};
//!
//! let rasterizer = Rasterizer::new(48.0);
//! let backend = AndroidBackend::new(80, 24);
//!
//! // Render to a pixel buffer (RGBA format)
//! let mut pixels = vec![0u8; 1920 * 1080 * 4];
//! rasterizer.render_to_surface(&backend, &mut pixels, 1920, 1920, 1080);
//! ```

use crate::backend::AndroidBackend;
use cosmic_text::{
    Attrs, Buffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use lazy_static::lazy_static;
use lru::LruCache;
use ratatui::style::Color;
use std::num::NonZeroUsize;
use std::sync::Mutex;

/// Cached character data: (width, height, is_wide, rgba_data)
pub type CachedChar = (u32, u32, bool, Vec<u8>);

lazy_static! {
    /// LRU cache for rendered characters to avoid repeated rendering
    /// Key: (char_code, size_u32, color_u32) -> Value: CachedChar
    pub static ref CHAR_CACHE: Mutex<LruCache<(u32, u32, u32), CachedChar>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap()));

    /// Global cosmic-text FontSystem - expensive to create, reuse across renders
    static ref FONT_SYSTEM: Mutex<FontSystem> = {
        log::info!("Initializing cosmic-text FontSystem...");

        #[allow(unused_mut)]
        let mut font_system = FontSystem::new();

        // Load Android system fonts on Android
        #[cfg(target_os = "android")]
        {
            load_android_system_fonts(&mut font_system);
        }

        log::info!("FontSystem initialized with {} font faces", font_system.db().len());
        Mutex::new(font_system)
    };

    /// Global SwashCache for glyph rasterization caching
    static ref SWASH_CACHE: Mutex<SwashCache> = {
        log::info!("Initializing cosmic-text SwashCache...");
        Mutex::new(SwashCache::new())
    };
}

/// Load Android system fonts into the FontSystem
#[cfg(target_os = "android")]
fn load_android_system_fonts(font_system: &mut FontSystem) {
    let font_paths = [
        // Monospace fonts (preferred for TUI)
        "/system/fonts/RobotoMono-Regular.ttf",
        "/system/fonts/DroidSansMono.ttf",
        "/system/fonts/CutiveMono.ttf",
        "/system/fonts/SourceCodePro-Regular.ttf",
        // Sans-serif fonts (fallback)
        "/system/fonts/Roboto-Regular.ttf",
        "/system/fonts/DroidSans.ttf",
        "/system/fonts/NotoSans-Regular.ttf",
        "/system/fonts/NotoSansSymbols-Regular-Subsetted.ttf",
        "/system/fonts/NotoSansSymbols-Regular-Subsetted2.ttf",
        // CJK fonts
        "/system/fonts/NotoSansCJK-Regular.ttc",
        "/system/fonts/NotoSerifCJK-Regular.ttc",
        "/system/fonts/DroidSansFallback.ttf",
        // Emoji fonts
        "/system/fonts/NotoColorEmoji.ttf",
        "/system/fonts/SamsungColorEmoji.ttf",
        // Symbol fonts
        "/system/fonts/NotoSansSymbols-Regular.ttf",
        "/system/fonts/AndroidEmoji.ttf",
    ];

    let mut loaded_count = 0;
    for path in &font_paths {
        match std::fs::read(path) {
            Ok(data) if data.len() > 100 => {
                log::info!("Loading font: {} ({} bytes)", path, data.len());
                font_system.db_mut().load_font_data(data);
                loaded_count += 1;
            }
            Ok(data) => {
                log::warn!("Font file too small: {} ({} bytes)", path, data.len());
            }
            Err(e) => {
                log::debug!("Font not found: {} ({:?})", path, e);
            }
        }
    }
    log::info!("Loaded {} font files from Android system", loaded_count);

    if loaded_count == 0 {
        log::error!("No Android system fonts could be loaded! Text rendering will fail.");
    }
}

/// Warm the cache by pre-rendering common ASCII characters.
/// Call this once at startup for better first-frame performance.
pub fn warm_cache(size: f32) {
    log::info!("Warming character cache for size {}", size);
    let white = [255u8, 255, 255, 255];

    // Pre-render printable ASCII characters (space to tilde)
    for c in ' '..='~' {
        let _ = render_char_cosmic(c, size, white);
    }

    // Pre-render common box-drawing characters
    let box_chars = [
        '─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼', '═', '║', '╔', '╗', '╚', '╝', '╠',
        '╣', '╦', '╩', '╬',
    ];
    for c in box_chars {
        let _ = render_char_cosmic(c, size, white);
    }

    if let Ok(cache) = CHAR_CACHE.lock() {
        log::info!("Cache warmed with {} characters", cache.len());
    }
}

/// Load custom font data into the global FontSystem.
pub fn load_font_data(data: Vec<u8>) {
    if data.len() > 100 {
        if let Ok(mut font_system) = FONT_SYSTEM.lock() {
            font_system.db_mut().load_font_data(data);
            log::info!("Loaded custom font into FontSystem");
        }
    }
}

/// Check if a character is considered "wide" (takes 2 terminal cells).
pub fn is_wide_char(c: char) -> bool {
    let code = c as u32;
    matches!(
        code,
        0x1100..=0x115F |   // Hangul Jamo
        0x2E80..=0x9FFF |   // CJK Unified Ideographs
        0xAC00..=0xD7A3 |   // Hangul Syllables
        0xF900..=0xFAFF |   // CJK Compatibility Ideographs
        0xFE10..=0xFE1F |   // Vertical forms
        0xFF00..=0xFFEF |   // Fullwidth forms
        0x1F300..=0x1F9FF | // Misc Symbols, Emoticons (includes emojis)
        0x1FA00..=0x1FAFF | // Extended symbols
        0x2600..=0x26FF |   // Misc symbols
        0x2700..=0x27BF |   // Dingbats
        0x1F1E0..=0x1F1FF   // Flags
    )
}

/// Render a single character using cosmic-text.
/// Returns (width, height, is_wide, rgba_data) if successful.
fn render_char_cosmic(c: char, size: f32, color: [u8; 4]) -> Option<CachedChar> {
    // Create cache key
    let size_u32 = size as u32;
    let color_u32 =
        ((color[3] as u32) << 24) | ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | (color[2] as u32);
    let cache_key = (c as u32, size_u32, color_u32);

    // Try to get from cache first
    {
        let mut cache = match CHAR_CACHE.lock() {
            Ok(cache) => cache,
            Err(e) => {
                log::warn!("Failed to lock character cache: {:?}", e);
                return None;
            }
        };
        if let Some(cached) = cache.get(&cache_key) {
            return Some((cached.0, cached.1, cached.2, cached.3.clone()));
        }
    }

    // Log first render attempt for diagnostic purposes
    static FIRST_CHAR_LOGGED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    let should_log_details = !FIRST_CHAR_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed);

    // Cache miss - render using cosmic-text
    let mut font_system = match FONT_SYSTEM.lock() {
        Ok(fs) => fs,
        Err(e) => {
            log::warn!("Failed to lock FontSystem: {:?}", e);
            return None;
        }
    };

    let mut swash_cache = match SWASH_CACHE.lock() {
        Ok(sc) => sc,
        Err(e) => {
            log::warn!("Failed to lock SwashCache: {:?}", e);
            return None;
        }
    };

    if should_log_details {
        log::info!(
            "First cosmic-text render: char='{}' (U+{:04X}), size={}, fonts_available={}",
            c,
            c as u32,
            size,
            font_system.db().len()
        );
    }

    // Determine if this is a wide character
    let is_wide = is_wide_char(c);

    // Calculate cell dimensions based on font size
    // IMPORTANT: These MUST match the grid spacing in Rasterizer
    let base_cell_width = (size * 0.6).ceil() as u32;
    let cell_width = if is_wide {
        base_cell_width * 2
    } else {
        base_cell_width
    };
    let cell_height = size.ceil() as u32;

    // Ensure minimum dimensions
    let cell_width = cell_width.max(4);
    let cell_height = cell_height.max(4);

    // Create metrics for the text
    let line_height = cell_height as f32;
    let render_size = size * 0.9;
    let metrics = Metrics::new(render_size, line_height);

    // Create a buffer with proper width for layout
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(&mut font_system, Some(cell_width as f32 * 2.0), Some(line_height));

    // Set text with monospace font family preference
    let attrs = Attrs::new().family(Family::Monospace);
    buffer.set_text(&mut font_system, &c.to_string(), attrs, Shaping::Advanced);

    // Shape the text to get layout
    buffer.shape_until_scroll(&mut font_system, false);

    // Create RGBA buffer for the character
    let mut rgba_data = vec![0u8; (cell_width * cell_height * 4) as usize];

    // Use the cosmic-text Color type for the draw callback
    let cosmic_color = CosmicColor::rgba(color[0], color[1], color[2], color[3]);

    // Track if we drew any pixels
    let mut pixels_drawn = 0u32;

    buffer.draw(
        &mut font_system,
        &mut swash_cache,
        cosmic_color,
        |x, y, _w, _h, pixel_color: CosmicColor| {
            if x < 0 || y < 0 || x >= cell_width as i32 || y >= cell_height as i32 {
                return;
            }

            let idx = ((y as u32 * cell_width + x as u32) * 4) as usize;
            if idx + 3 >= rgba_data.len() {
                return;
            }

            let r = pixel_color.r();
            let g = pixel_color.g();
            let b = pixel_color.b();
            let a = pixel_color.a();

            if a == 0 {
                return;
            }

            pixels_drawn += 1;

            // Alpha blend
            let alpha_f = a as f32 / 255.0;
            let bg_alpha = 1.0 - alpha_f;

            rgba_data[idx] = ((rgba_data[idx] as f32 * bg_alpha) + (r as f32 * alpha_f)) as u8;
            rgba_data[idx + 1] =
                ((rgba_data[idx + 1] as f32 * bg_alpha) + (g as f32 * alpha_f)) as u8;
            rgba_data[idx + 2] =
                ((rgba_data[idx + 2] as f32 * bg_alpha) + (b as f32 * alpha_f)) as u8;
            rgba_data[idx + 3] = rgba_data[idx + 3].max(a);
        },
    );

    if should_log_details {
        log::info!(
            "  Rendered: cell_size={}x{}, pixels_drawn={}",
            cell_width,
            cell_height,
            pixels_drawn
        );
    }

    // If no content was rendered, return None to trigger fallback
    if pixels_drawn == 0 {
        static NO_GLYPH_COUNT: std::sync::atomic::AtomicU32 =
            std::sync::atomic::AtomicU32::new(0);
        let count = NO_GLYPH_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count < 10 {
            log::warn!("No pixels drawn for '{}' (U+{:04X})", c, c as u32);
        }
        return None;
    }

    let result = (cell_width, cell_height, is_wide, rgba_data);

    // Cache the result
    if let Ok(mut cache) = CHAR_CACHE.lock() {
        cache.put(cache_key, result.clone());
    }

    Some(result)
}

/// Software rasterizer that converts Ratatui cells to pixels.
pub struct Rasterizer {
    font_width: f32,
    font_height: f32,
    font_size: f32,
}

impl Rasterizer {
    /// Create a new rasterizer with the specified font size.
    ///
    /// # Arguments
    ///
    /// * `size` - Font size in pixels
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui_android::Rasterizer;
    ///
    /// let rasterizer = Rasterizer::new(48.0);
    /// assert_eq!(rasterizer.font_height(), 48.0);
    /// ```
    pub fn new(size: f32) -> Self {
        Self {
            font_width: (size * 0.6).ceil(),
            font_height: size.ceil(),
            font_size: size,
        }
    }

    /// Get the font width (character cell width in pixels).
    pub fn font_width(&self) -> f32 {
        self.font_width
    }

    /// Get the font height (character cell height in pixels).
    pub fn font_height(&self) -> f32 {
        self.font_height
    }

    /// Get the configured font size.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Renders the backend's cell buffer to a pixel surface.
    ///
    /// # Arguments
    ///
    /// * `backend` - The Ratatui backend containing cell data
    /// * `dest` - Destination pixel buffer (RGBA format)
    /// * `stride` - Row stride in pixels (usually same as width)
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    pub fn render_to_surface(
        &self,
        backend: &AndroidBackend,
        dest: &mut [u8],
        stride: usize,
        window_width: usize,
        window_height: usize,
    ) {
        self.render_to_surface_with_offset(backend, dest, stride, window_width, window_height, 0, 0);
    }

    /// Render with vertical offsets (for status bar and navigation bar).
    ///
    /// # Arguments
    ///
    /// * `backend` - The Ratatui backend containing cell data
    /// * `dest` - Destination pixel buffer (RGBA format)
    /// * `stride` - Row stride in pixels
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    /// * `top_offset_px` - Top offset in pixels (e.g., for status bar)
    /// * `bottom_offset_px` - Bottom offset in pixels (e.g., for navigation bar + keyboard)
    pub fn render_to_surface_with_offset(
        &self,
        backend: &AndroidBackend,
        dest: &mut [u8],
        stride: usize,
        window_width: usize,
        window_height: usize,
        top_offset_px: usize,
        bottom_offset_px: usize,
    ) {
        // Safety check
        let expected_buffer_size = stride * window_height * 4;
        if dest.len() < expected_buffer_size {
            log::warn!(
                "Buffer too small! dest.len()={}, expected={}",
                dest.len(),
                expected_buffer_size
            );
            return;
        }

        let buffer_area = backend.buffer_area();
        let buffer_width = buffer_area.width;
        let buffer_height = buffer_area.height;

        let max_render_height = window_height.saturating_sub(bottom_offset_px);

        for term_y in 0..buffer_height {
            let py_start = (term_y as f32 * self.font_height) as usize + top_offset_px;

            if py_start >= max_render_height {
                continue;
            }

            let py_end_content = ((term_y + 1) as f32 * self.font_height) as usize;
            let py_end = py_end_content.saturating_add(top_offset_px);
            let row_height = (py_end - py_start).min(max_render_height.saturating_sub(py_start));

            if row_height == 0 {
                continue;
            }

            let mut skip_until_x: u16 = 0;

            for term_x in 0..buffer_width {
                if term_x < skip_until_x {
                    continue;
                }

                let cell = backend
                    .get_cell(term_x, term_y)
                    .cloned()
                    .unwrap_or_default();

                let symbol = cell.symbol();
                let px_start = (term_x as f32 * self.font_width) as usize;

                if px_start >= window_width {
                    continue;
                }

                let first_ch = symbol.chars().next().unwrap_or(' ');
                let char_is_wide = is_wide_char(first_ch);

                let cell_multiplier = if char_is_wide { 2.0 } else { 1.0 };
                let px_end = ((term_x as f32 + cell_multiplier) * self.font_width) as usize;
                let cell_width = (px_end - px_start).min(window_width - px_start);

                if cell_width == 0 {
                    continue;
                }

                if char_is_wide {
                    skip_until_x = term_x + 2;
                }

                let fg_color = color_to_rgba(cell.fg);
                let bg_color = color_to_rgba_bg(cell.bg);

                // Render background
                self.render_cell_background(
                    dest,
                    stride,
                    window_width,
                    window_height,
                    px_start,
                    py_start,
                    cell_width,
                    row_height,
                    bg_color,
                );

                // Render character(s)
                if !symbol.is_empty() && first_ch != ' ' {
                    let mut char_x = px_start;
                    for ch in symbol.chars() {
                        if ch == ' ' {
                            char_x += self.font_width as usize;
                            continue;
                        }
                        if char_x < px_start + cell_width {
                            self.draw_char(
                                ch,
                                char_x,
                                py_start,
                                fg_color,
                                dest,
                                stride,
                                window_width,
                                window_height,
                            );
                            if is_wide_char(ch) {
                                char_x += (self.font_width * 2.0) as usize;
                            } else {
                                char_x += self.font_width as usize;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn render_cell_background(
        &self,
        dest: &mut [u8],
        stride: usize,
        window_width: usize,
        window_height: usize,
        px_start: usize,
        py_start: usize,
        cell_width: usize,
        row_height: usize,
        bg_color: [u8; 4],
    ) {
        for py_offset in 0..row_height {
            let py = py_start + py_offset;
            if py >= window_height {
                break;
            }

            for px_offset in 0..cell_width {
                let px = px_start + px_offset;
                if px >= window_width {
                    break;
                }

                let idx = (py * stride + px) * 4;
                if idx + 3 < dest.len() {
                    dest[idx] = bg_color[0];
                    dest[idx + 1] = bg_color[1];
                    dest[idx + 2] = bg_color[2];
                    dest[idx + 3] = bg_color[3];
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
        window_width: usize,
        window_height: usize,
    ) {
        // Try cosmic-text rendering first
        if let Some((width, height, _is_wide, rgba_data)) = render_char_cosmic(c, self.font_size, color) {
            if width > 0 && height > 0 && !rgba_data.is_empty() {
                let cell_width_multiplier = if is_wide_char(c) { 2.0 } else { 1.0 };
                self.render_bitmap(
                    width,
                    height,
                    &rgba_data,
                    px,
                    py,
                    dest,
                    stride,
                    window_width,
                    window_height,
                    cell_width_multiplier,
                );
                return;
            }
        }

        // Android native rendering fallback would go here (feature-gated)
        #[cfg(all(target_os = "android", feature = "android-native-backend"))]
        {
            if let Some((width, height, _is_wide, rgba_data)) =
                crate::android_render::render_char_android(c, self.font_size, color)
            {
                if width > 0 && height > 0 && !rgba_data.is_empty() {
                    let cell_width_multiplier = if is_wide_char(c) { 2.0 } else { 1.0 };
                    self.render_bitmap(
                        width,
                        height,
                        &rgba_data,
                        px,
                        py,
                        dest,
                        stride,
                        window_width,
                        window_height,
                        cell_width_multiplier,
                    );
                    return;
                }
            }
        }

        // Fallback: draw a simple block
        self.draw_fallback_block(px, py, color, dest, stride, window_width, window_height);
    }

    fn render_bitmap(
        &self,
        width: u32,
        height: u32,
        rgba_data: &[u8],
        px: usize,
        py: usize,
        dest: &mut [u8],
        stride: usize,
        window_width: usize,
        window_height: usize,
        cell_width_multiplier: f32,
    ) {
        let bitmap_width = width as usize;
        let bitmap_height = height as usize;

        let expected_size = bitmap_width * bitmap_height * 4;
        if rgba_data.len() < expected_size {
            return;
        }

        let cell_width = (self.font_width * cell_width_multiplier) as usize;
        let cell_height = self.font_height as usize;

        let scale_x = cell_width as f32 / bitmap_width as f32;
        let scale_y = cell_height as f32 / bitmap_height as f32;
        let scale = scale_x.min(scale_y).min(1.0);

        let scaled_width = (bitmap_width as f32 * scale) as usize;
        let scaled_height = (bitmap_height as f32 * scale) as usize;

        let offset_x = (cell_width.saturating_sub(scaled_width)) / 2;

        for dest_y_rel in 0..scaled_height {
            let dest_y = py + dest_y_rel;
            if dest_y >= window_height {
                continue;
            }

            let src_y = (dest_y_rel as f32 / scale) as usize;
            if src_y >= bitmap_height {
                continue;
            }

            for dest_x_rel in 0..scaled_width {
                let dest_x = px + offset_x + dest_x_rel;
                if dest_x >= window_width {
                    continue;
                }

                let src_x = (dest_x_rel as f32 / scale) as usize;
                if src_x >= bitmap_width {
                    continue;
                }

                let src_idx = (src_y * bitmap_width + src_x) * 4;
                if src_idx + 3 >= rgba_data.len() {
                    continue;
                }

                let r = rgba_data[src_idx];
                let g = rgba_data[src_idx + 1];
                let b = rgba_data[src_idx + 2];
                let alpha = rgba_data[src_idx + 3];

                if alpha == 0 {
                    continue;
                }

                let idx = (dest_y * stride + dest_x) * 4;
                if idx + 3 >= dest.len() {
                    continue;
                }

                let alpha_f = alpha as f32 / 255.0;
                let bg_alpha = 1.0 - alpha_f;

                dest[idx] = ((dest[idx] as f32 * bg_alpha) + (r as f32 * alpha_f)) as u8;
                dest[idx + 1] = ((dest[idx + 1] as f32 * bg_alpha) + (g as f32 * alpha_f)) as u8;
                dest[idx + 2] = ((dest[idx + 2] as f32 * bg_alpha) + (b as f32 * alpha_f)) as u8;
                dest[idx + 3] = dest[idx + 3].max(alpha);
            }
        }
    }

    fn draw_fallback_block(
        &self,
        px: usize,
        py: usize,
        color: [u8; 4],
        dest: &mut [u8],
        stride: usize,
        window_width: usize,
        window_height: usize,
    ) {
        let block_size = (self.font_height * 0.6).max(4.0) as usize;
        let start_x = px + ((self.font_width as usize).saturating_sub(block_size)) / 2;
        let start_y = py + ((self.font_height as usize).saturating_sub(block_size)) / 2;

        if start_x >= window_width || start_y >= window_height {
            return;
        }

        let end_x = (start_x + block_size).min(window_width);
        let end_y = (start_y + block_size).min(window_height);

        for y in start_y..end_y {
            for x in start_x..end_x {
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
}

/// Convert Ratatui color to RGBA.
pub fn color_to_rgba(color: Color) -> [u8; 4] {
    match color {
        Color::Reset => [255, 255, 255, 255],
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
        Color::Indexed(_) => [255, 255, 255, 255],
    }
}

/// Convert Ratatui background color to RGBA.
pub fn color_to_rgba_bg(color: Color) -> [u8; 4] {
    match color {
        Color::Reset => [0, 0, 0, 255],
        _ => color_to_rgba(color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterizer_new() {
        let rasterizer = Rasterizer::new(48.0);
        assert_eq!(rasterizer.font_height(), 48.0);
        assert_eq!(rasterizer.font_width(), 29.0); // ceil(48 * 0.6) = ceil(28.8) = 29
    }

    #[test]
    fn test_is_wide_char() {
        assert!(is_wide_char('中'));
        assert!(is_wide_char('日'));
        assert!(is_wide_char('🎉'));
        assert!(!is_wide_char('a'));
        assert!(!is_wide_char('1'));
    }

    #[test]
    fn test_color_to_rgba() {
        assert_eq!(color_to_rgba(Color::Red), [255, 0, 0, 255]);
        assert_eq!(color_to_rgba(Color::Rgb(100, 150, 200)), [100, 150, 200, 255]);
    }
}

