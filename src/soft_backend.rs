//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use core::fmt::{self, Write};
use core::iter;
use std::io;
use unicode_width::UnicodeWidthStr;

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SoftBackend {
    buffer: Buffer,

    cursor: bool,
    pos: (u16, u16),
}

impl SoftBackend {
    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),

            cursor: false,
            pos: (0, 0),
        }
    }

    /// Returns a reference to the internal buffer of the `SoftBackend`.
    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Resizes the `SoftBackend` to the specified width and height.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.buffer.resize(Rect::new(0, 0, width, height));
    }
}

impl Backend for SoftBackend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, c) in content {
            self.buffer[(x, y)] = c.clone();
            println!("{c:#?}");
        }

        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.cursor = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.cursor = true;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.pos.into())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.pos = position.into().into();
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.buffer.reset();
        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.buffer.area.as_size())
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        // Some arbitrary window pixel size, probably doesn't need much testing.
        const WINDOW_PIXEL_SIZE: Size = Size {
            width: 640,
            height: 480,
        };
        Ok(WindowSize {
            columns_rows: self.buffer.area.as_size(),
            pixels: WINDOW_PIXEL_SIZE,
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
