//! Custom Ratatui backend for Android that renders to an in-memory cell buffer.
//!
//! This backend acts as a terminal emulator, storing the TUI state in memory
//! instead of writing ANSI codes to stdout. The rasterizer then reads this
//! buffer to render pixels to the Android Surface.
//!
//! ## Usage
//!
//! ```rust
//! use ratatui_android::AndroidBackend;
//! use ratatui::Terminal;
//!
//! let backend = AndroidBackend::new(80, 24);
//! let mut terminal = Terminal::new(backend).unwrap();
//!
//! terminal.draw(|frame| {
//!     // Draw your UI here
//! }).unwrap();
//! ```

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use std::io;

/// A Ratatui backend that renders to an in-memory grid of cells.
///
/// This backend stores all cell data in memory, allowing the rasterizer
/// to read the buffer and convert it to pixels for Android Surface rendering.
///
/// ## Thread Safety
///
/// The backend is not thread-safe. Access from multiple threads requires
/// external synchronization.
pub struct AndroidBackend {
    /// Terminal width in columns
    pub width: u16,
    /// Terminal height in rows
    pub height: u16,
    cursor_x: u16,
    cursor_y: u16,
    /// The grid of cells representing the screen state
    pub buffer: Buffer,
}

impl AndroidBackend {
    /// Create a new Android backend with the specified dimensions.
    ///
    /// # Arguments
    ///
    /// * `width` - Number of columns
    /// * `height` - Number of rows
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui_android::AndroidBackend;
    ///
    /// let backend = AndroidBackend::new(80, 24);
    /// assert_eq!(backend.width, 80);
    /// assert_eq!(backend.height, 24);
    /// ```
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
        }
    }

    /// Resize the backend to new dimensions.
    ///
    /// This clears the buffer and allocates a new one with the specified size.
    ///
    /// # Arguments
    ///
    /// * `width` - New number of columns
    /// * `height` - New number of rows
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.buffer.resize(Rect::new(0, 0, width, height));
    }

    /// Get a reference to a cell at the specified position.
    ///
    /// Returns `None` if the position is out of bounds.
    ///
    /// # Arguments
    ///
    /// * `x` - Column (0-indexed)
    /// * `y` - Row (0-indexed)
    pub fn get_cell(&self, x: u16, y: u16) -> Option<&Cell> {
        if x < self.width && y < self.height {
            self.buffer.cell((x, y))
        } else {
            None
        }
    }

    /// Get a mutable reference to a cell at the specified position.
    ///
    /// Returns `None` if the position is out of bounds.
    pub fn get_cell_mut(&mut self, x: u16, y: u16) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            self.buffer.cell_mut((x, y))
        } else {
            None
        }
    }

    /// Get the buffer's area (for iterating over all cells).
    pub fn buffer_area(&self) -> Rect {
        *self.buffer.area()
    }

    /// Get a reference to the underlying buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Iterate over all cells in the buffer.
    pub fn cells(&self) -> impl Iterator<Item = (u16, u16, &Cell)> {
        let area = self.buffer_area();
        (0..area.height).flat_map(move |y| {
            (0..area.width).filter_map(move |x| {
                self.get_cell(x, y).map(|cell| (x, y, cell))
            })
        })
    }
}

impl Backend for AndroidBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            if x < self.width && y < self.height {
                if let Some(buffer_cell) = self.buffer.cell_mut((x, y)) {
                    *buffer_cell = cell.clone();
                }
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        // Cursor is not rendered on Android
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        // Cursor is not rendered on Android
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        Ok(Position::new(self.cursor_x, self.cursor_y))
    }

    fn set_cursor_position<P: Into<Position>>(
        &mut self,
        position: P,
    ) -> Result<(), Self::Error> {
        let pos = position.into();
        self.cursor_x = pos.x;
        self.cursor_y = pos.y;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.buffer.reset();
        Ok(())
    }

    fn clear_region(&mut self, _clear_type: ClearType) -> Result<(), Self::Error> {
        // For Android, we just clear the whole buffer
        // A more sophisticated implementation could clear specific regions
        self.buffer.reset();
        Ok(())
    }

    fn size(&self) -> Result<Size, Self::Error> {
        Ok(Size::new(self.width, self.height))
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        // Return approximate pixel size based on typical character dimensions
        Ok(WindowSize {
            columns_rows: Size::new(self.width, self.height),
            pixels: Size::new(
                (self.width as u32 * 8) as u16,
                (self.height as u32 * 16) as u16,
            ),
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // In a normal terminal, this writes to stdout.
        // Here, it does nothing. We trigger the render loop manually.
        Ok(())
    }
}

impl Default for AndroidBackend {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    #[test]
    fn test_backend_new() {
        let backend = AndroidBackend::new(120, 40);
        assert_eq!(backend.width, 120);
        assert_eq!(backend.height, 40);
    }

    #[test]
    fn test_backend_resize() {
        let mut backend = AndroidBackend::new(80, 24);
        backend.resize(100, 50);
        assert_eq!(backend.width, 100);
        assert_eq!(backend.height, 50);
    }

    #[test]
    fn test_backend_get_cell() {
        let backend = AndroidBackend::new(80, 24);
        assert!(backend.get_cell(0, 0).is_some());
        assert!(backend.get_cell(79, 23).is_some());
        assert!(backend.get_cell(80, 24).is_none());
    }

    #[test]
    fn test_backend_draw() {
        let mut backend = AndroidBackend::new(80, 24);
        let mut cell = Cell::default();
        cell.set_char('X');
        cell.set_style(Style::default().fg(Color::Red));

        backend.draw([(5, 5, &cell)].into_iter()).unwrap();

        let drawn_cell = backend.get_cell(5, 5).unwrap();
        assert_eq!(drawn_cell.symbol(), "X");
    }

    #[test]
    fn test_backend_clear() {
        let mut backend = AndroidBackend::new(80, 24);
        let mut cell = Cell::default();
        cell.set_char('X');

        backend.draw([(0, 0, &cell)].into_iter()).unwrap();
        backend.clear().unwrap();

        let cleared_cell = backend.get_cell(0, 0).unwrap();
        assert_eq!(cleared_cell.symbol(), " ");
    }

    #[test]
    fn test_backend_size() {
        let backend = AndroidBackend::new(100, 50);
        let size = backend.size().unwrap();
        assert_eq!(size.width, 100);
        assert_eq!(size.height, 50);
    }

    #[test]
    fn test_backend_cells_iterator() {
        let backend = AndroidBackend::new(10, 10);
        let cells: Vec<_> = backend.cells().collect();
        assert_eq!(cells.len(), 100);
    }
}

