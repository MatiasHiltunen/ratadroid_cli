//! Custom Ratatui backend for Android that renders to an in-memory cell buffer.
//! This backend acts as a terminal emulator, storing the TUI state in memory
//! instead of writing ANSI codes to stdout.

use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use std::io;

/// A Ratatui backend that renders to an in-memory grid of cells.
/// The rasterizer will read this buffer to draw pixels to the Android Surface.
pub struct AndroidBackend {
    pub width: u16,
    pub height: u16,
    cursor_x: u16,
    cursor_y: u16,
    /// The grid of cells representing the screen state
    pub buffer: Buffer,
}

impl AndroidBackend {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.buffer.resize(Rect::new(0, 0, width, height));
    }
    
    pub fn get_cell(&self, x: u16, y: u16) -> Option<&Cell> {
        if x < self.width && y < self.height {
            Some(self.buffer.get(x, y))
        } else {
            None
        }
    }
}

impl Backend for AndroidBackend {
    fn draw<'a, I>(&mut self, content: I) -> Result<(), io::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            if x < self.width && y < self.height {
                *self.buffer.get_mut(x, y) = cell.clone();
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), io::Error> { 
        Ok(()) 
    }
    
    fn show_cursor(&mut self) -> Result<(), io::Error> { 
        Ok(()) 
    }
    
    fn get_cursor(&mut self) -> Result<(u16, u16), io::Error> {
        Ok((self.cursor_x, self.cursor_y))
    }
    
    fn set_cursor(&mut self, x: u16, y: u16) -> Result<(), io::Error> {
        self.cursor_x = x;
        self.cursor_y = y;
        Ok(())
    }
    
    fn clear(&mut self) -> Result<(), io::Error> {
        self.buffer.reset();
        Ok(())
    }
    
    fn size(&self) -> Result<Rect, io::Error> {
        Ok(Rect::new(0, 0, self.width, self.height))
    }
    
    fn window_size(&mut self) -> Result<WindowSize, io::Error> {
        use ratatui::layout::Size;
        Ok(WindowSize {
            columns_rows: Size {
                width: self.width,
                height: self.height,
            },
            pixels: Size {
                width: self.width as u32 * 8,
                height: self.height as u32 * 16,
            },
        })
    }
    
    fn flush(&mut self) -> Result<(), io::Error> {
        // In a normal terminal, this writes to stdout.
        // Here, it does nothing. We trigger the render loop manually.
        Ok(())
    }
}

