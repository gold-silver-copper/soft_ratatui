//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

use cosmic_text::Style;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color as RatColor, Modifier};

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight,
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
    glyph_width: f32,
    glyph_height: f32,
    line_height: u32,
}

fn add_strikeout(text: &String) -> String {
    // Unicode combining long stroke overlay
    let strike = '\u{0336}';
    text.chars().flat_map(|c| [c, strike]).collect()
}
fn add_underline(text: &String) -> String {
    // Unicode combining long stroke overlay
    let strike = '\u{0332}';
    text.chars().flat_map(|c| [c, strike]).collect()
}

impl SoftBackend {
    pub fn draw_cell(&mut self, rat_cell: &Cell, xik: u16, yik: u16) {
        let mut buffer = CosmicBuffer::new(&mut self.font_system, self.metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);

        // Set a size for the text buffer, in pixels
        let width = self.glyph_width + 1.00;
        let height = self.line_height;
        buffer.set_size(Some(width as f32), Some(height as f32));

        // Prepare Pixmap to draw into

        // Draw using tiny-skia
        let mut swash_cache = SwashCache::new();
        let mut paint = Paint::default();
        let mut pixmap = Pixmap::new(width as u32, height).unwrap();

        let is_bold = rat_cell.modifier.contains(Modifier::BOLD);
        let is_italic = rat_cell.modifier.contains(Modifier::ITALIC);
        let is_underlined = rat_cell.modifier.contains(Modifier::UNDERLINED);
        let is_slowblink = rat_cell.modifier.contains(Modifier::SLOW_BLINK);
        let is_rapidblink = rat_cell.modifier.contains(Modifier::RAPID_BLINK);
        let is_reversed = rat_cell.modifier.contains(Modifier::REVERSED);
        let is_dim = rat_cell.modifier.contains(Modifier::DIM);
        let is_hidden = rat_cell.modifier.contains(Modifier::HIDDEN);
        let is_crossed_out = rat_cell.modifier.contains(Modifier::CROSSED_OUT);

        let mut rat_fg = rat_cell.fg.clone();
        let rat_bg = rat_cell.bg.clone();
        if is_hidden {
            rat_fg = rat_bg.clone();
        }

        let mut text_color = if is_reversed {
            pixmap.fill(rat_to_skia_color(&rat_fg, true));

            rat_to_cosmic_color(&rat_bg, false)
        } else {
            pixmap.fill(rat_to_skia_color(&rat_bg, false));

            rat_to_cosmic_color(&rat_fg, true)
        };

        let mut text_symbol: String = rat_cell.symbol().to_string();

        if is_crossed_out {
            text_symbol = add_strikeout(&text_symbol);
        }
        if is_underlined {
            text_symbol = add_underline(&text_symbol);
        }

        let mut attrs = Attrs::new().family(Family::Monospace);
        if is_bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if is_italic {
            attrs = attrs.style(Style::Italic);
        }
        // Set and shape text
        buffer.set_text(&text_symbol, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(true);
        buffer.draw(&mut swash_cache, text_color, |x, y, w, h, color| {
            if let Some(rect) = SkiaRect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
                let [r, g, b, a] = color.as_rgba();
                paint.set_color(SkiaColor::from_rgba8(r, g, b, a));
                pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            }
        });
        let mut paint = PixmapPaint::default();

        paint.quality = FilterQuality::Nearest;
        self.pixmapik.draw_pixmap(
            (xik as f32 * self.glyph_width) as i32,
            (yik as f32 * self.metrics.line_height) as i32,
            pixmap.as_ref(),
            &paint,
            Transform::identity(),
            None,
        );
    }

    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let line_height = 20;
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(line_height as f32, line_height as f32);
        let mut buffer = CosmicBuffer::new(&mut font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut font_system);

        buffer.set_text(
            "█\n█",
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(true);
        /* for runik in buffer.layout_runs() {
            println!("Glyph height (bbox): {:#?}", runik);
            let glyph_width = runik.line_w;
            let glyph_height = runik.line_y;
        } */

        let boop = buffer.layout_runs().next().unwrap();
        let glyph_width = boop.line_w;
        let glyph_height = boop.line_y;
        println!("Glyph height (bbox): {:#?}", boop);

        let mut pixmapik = Pixmap::new(
            (glyph_width * width as f32) as u32,
            (metrics.line_height * height as f32) as u32,
        )
        .unwrap();

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,
            metrics,
            pixmapik,
            glyph_width,
            glyph_height,
            line_height,
        };
        return_struct.clear();
        return_struct
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
            self.draw_cell(&c, x, y);
            //   println!("{c:#?}");
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
        let clear_cell = Cell::EMPTY;
        for x in (0..self.buffer.area.width) {
            for y in (0..self.buffer.area.height) {
                self.draw_cell(&clear_cell, x, y);
            }
        }
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

pub fn rat_to_cosmic_color(rat_col: &RatColor, is_a_fg: bool) -> CosmicColor {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                CosmicColor::rgba(204, 204, 255, 255)
            } else {
                CosmicColor::rgba(15, 15, 112, 255)
            }
        }
        RatColor::Black => CosmicColor::rgba(0, 0, 0, 255),
        RatColor::Red => CosmicColor::rgba(139, 0, 0, 255),
        RatColor::Green => CosmicColor::rgba(0, 100, 0, 255),
        RatColor::Yellow => CosmicColor::rgba(255, 215, 0, 255),
        RatColor::Blue => CosmicColor::rgba(0, 0, 139, 255),
        RatColor::Magenta => CosmicColor::rgba(99, 9, 99, 255),
        RatColor::Cyan => CosmicColor::rgba(0, 0, 255, 255),
        RatColor::Gray => CosmicColor::rgba(128, 128, 128, 255),
        RatColor::DarkGray => CosmicColor::rgba(64, 64, 64, 255),
        RatColor::LightRed => CosmicColor::rgba(255, 0, 0, 255),
        RatColor::LightGreen => CosmicColor::rgba(0, 255, 0, 255),
        RatColor::LightBlue => CosmicColor::rgba(173, 216, 230, 255),
        RatColor::LightYellow => CosmicColor::rgba(255, 255, 224, 255),
        RatColor::LightMagenta => CosmicColor::rgba(139, 0, 139, 255),
        RatColor::LightCyan => CosmicColor::rgba(224, 255, 255, 255),
        RatColor::White => CosmicColor::rgba(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            let i = *i as u8;
            CosmicColor::rgba(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => CosmicColor::rgba(*r, *g, *b, 255),
    }
}
