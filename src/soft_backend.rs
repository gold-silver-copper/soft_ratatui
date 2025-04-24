//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::Color as RatColor;

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, FontSystem, Metrics, Shaping, SwashCache,
};
use tiny_skia::{ColorU8 as SkiaColor, Paint, Pixmap, Rect as SkiaRect};

#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font_system: FontSystem,
    metrics: Metrics,
}

impl SoftBackend {
    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let font_system = FontSystem::new();

        let metrics = Metrics::new(14.0, 20.0);
        Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,
            metrics,
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

pub fn cell_to_skia(cell: &Cell) -> i64 {
    5
}

/// Convert a ratatui color to a tiny-skia SkiaColor
pub fn rat_to_skia_color(rat_col: &RatColor, is_a_fg: bool) -> SkiaColor {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                SkiaColor::from_rgba(204, 204, 255, 255)
            } else {
                SkiaColor::from_rgba(15, 15, 112, 255)
            }
        }
        RatColor::Black => SkiaColor::from_rgba(0, 0, 0, 255),
        RatColor::Red => SkiaColor::from_rgba(139, 0, 0, 255),
        RatColor::Green => SkiaColor::from_rgba(0, 100, 0, 255),
        RatColor::Yellow => SkiaColor::from_rgba(255, 215, 0, 255),
        RatColor::Blue => SkiaColor::from_rgba(0, 0, 139, 255),
        RatColor::Magenta => SkiaColor::from_rgba(99, 9, 99, 255),
        RatColor::Cyan => SkiaColor::from_rgba(0, 0, 255, 255),
        RatColor::Gray => SkiaColor::from_rgba(128, 128, 128, 255),
        RatColor::DarkGray => SkiaColor::from_rgba(64, 64, 64, 255),
        RatColor::LightRed => SkiaColor::from_rgba(255, 0, 0, 255),
        RatColor::LightGreen => SkiaColor::from_rgba(0, 255, 0, 255),
        RatColor::LightBlue => SkiaColor::from_rgba(173, 216, 230, 255),
        RatColor::LightYellow => SkiaColor::from_rgba(255, 255, 224, 255),
        RatColor::LightMagenta => SkiaColor::from_rgba(139, 0, 139, 255),
        RatColor::LightCyan => SkiaColor::from_rgba(224, 255, 255, 255),
        RatColor::White => SkiaColor::from_rgba(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            // You can customize this mapping, or use a fixed 256-color palette lookup
            let i = *i as u8;
            SkiaColor::from_rgba(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => SkiaColor::from_rgba(*r, *g, *b, 255),
    }
}
