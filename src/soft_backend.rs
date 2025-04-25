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
use tiny_skia::{
    Color as SkiaColor, FilterQuality, Paint, Pixmap, PixmapMut, PixmapPaint, Rect as SkiaRect,
    Transform,
};

#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font_system: FontSystem,
    metrics: Metrics,
    pixmapik: Pixmap,
}

pub fn draw_cell(
    rat_cell: &Cell,
    font_system: &mut FontSystem,
    metrics: Metrics,
    pixmapik: &mut Pixmap,
    xik: u16,
    yik: u16,
) {
    let mut buffer = CosmicBuffer::new(font_system, metrics);
    let mut buffer = buffer.borrow_with(font_system);

    // Set a size for the text buffer, in pixels
    let width = 160;
    let height = 50;
    buffer.set_size(Some(width as f32), Some(height as f32));

    // Set and shape text
    buffer.set_text(rat_cell.symbol(), &Attrs::new(), Shaping::Advanced);
    buffer.shape_until_scroll(true);

    // Prepare Pixmap to draw into
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(rat_to_skia_color(&rat_cell.bg, false)); // black background

    let text_color = CosmicColor::rgb(0b1111, 0xFF, 0xFF); // white

    // Draw using tiny-skia
    let mut swash_cache = SwashCache::new();
    buffer.draw(&mut swash_cache, text_color, |x, y, w, h, _color| {
        if let Some(rect) = SkiaRect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
            let mut paint = Paint::default();

            paint.set_color(rat_to_skia_color(&rat_cell.fg, true));
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        }
    });
    let mut paint = PixmapPaint::default();

    paint.quality = FilterQuality::Bicubic;
    pixmapik.draw_pixmap(
        (xik * 15) as i32,
        (yik * 15) as i32,
        pixmap.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
}

impl SoftBackend {
    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let font_system = FontSystem::new();
        let metrics = Metrics::new(14.0, 20.0);
        let mut pixmapik = Pixmap::new(160, 50).unwrap();

        Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,
            metrics,
            pixmapik,
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
            draw_cell(
                &c,
                &mut self.font_system,
                self.metrics.clone(),
                &mut self.pixmapik,
                x,
                y,
            );
            println!("{c:#?}");
        }
        // Save to PNG file
        self.pixmapik.save_png("output_tiny_skia.png").unwrap();

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
                SkiaColor::from_rgba8(204, 204, 255, 255)
            } else {
                SkiaColor::from_rgba8(15, 15, 112, 255)
            }
        }
        RatColor::Black => SkiaColor::from_rgba8(0, 0, 0, 255),
        RatColor::Red => SkiaColor::from_rgba8(139, 0, 0, 255),
        RatColor::Green => SkiaColor::from_rgba8(0, 100, 0, 255),
        RatColor::Yellow => SkiaColor::from_rgba8(255, 215, 0, 255),
        RatColor::Blue => SkiaColor::from_rgba8(0, 0, 139, 255),
        RatColor::Magenta => SkiaColor::from_rgba8(99, 9, 99, 255),
        RatColor::Cyan => SkiaColor::from_rgba8(0, 0, 255, 255),
        RatColor::Gray => SkiaColor::from_rgba8(128, 128, 128, 255),
        RatColor::DarkGray => SkiaColor::from_rgba8(64, 64, 64, 255),
        RatColor::LightRed => SkiaColor::from_rgba8(255, 0, 0, 255),
        RatColor::LightGreen => SkiaColor::from_rgba8(0, 255, 0, 255),
        RatColor::LightBlue => SkiaColor::from_rgba8(173, 216, 230, 255),
        RatColor::LightYellow => SkiaColor::from_rgba8(255, 255, 224, 255),
        RatColor::LightMagenta => SkiaColor::from_rgba8(139, 0, 139, 255),
        RatColor::LightCyan => SkiaColor::from_rgba8(224, 255, 255, 255),
        RatColor::White => SkiaColor::from_rgba8(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            // You can customize this mapping, or use a fixed 256-color palette lookup
            let i = *i as u8;
            SkiaColor::from_rgba8(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => SkiaColor::from_rgba8(*r, *g, *b, 255),
    }
}
