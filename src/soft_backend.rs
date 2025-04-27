//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

use crate::colors::*;

use fontdue::Font;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color as RatColor, Modifier};
use tiny_skia::{
    BlendMode, Color as SkiaColor, ColorU8 as SkiaColorU8, FilterQuality, IntSize, Paint, Pixmap,
    PixmapMut, PixmapPaint, PremultipliedColorU8, Rect as SkiaRect, Transform,
};
static FONT_DATA: &[u8] = include_bytes!("../assets/iosevka.ttf");
#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font_size: f32,
    font: Font,
    screen_pixmap: Pixmap,

    pixmap_paint: PixmapPaint,

    char_width: u32,
    char_height: u32,
    glyph_pixmap: Pixmap,
    skia_paint: Paint<'static>,
}

impl SoftBackend {
    pub fn draw_cell(&mut self, rat_cell: &Cell, xik: u16, yik: u16) {
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

        let mut mut_pixmap = self.glyph_pixmap.as_mut();

        let mut text_color = if is_reversed {
            mut_pixmap.fill(rat_to_skia_color(&rat_fg, true));

            rat_to_skia_color(&rat_bg, false)
        } else {
            mut_pixmap.fill(rat_to_skia_color(&rat_bg, false));

            rat_to_skia_color(&rat_fg, true)
        };

        // Rasterize each character
        let (metrics, bitmap) = self
            .font
            .rasterize(rat_cell.symbol().chars().next().unwrap(), self.font_size);
        self.skia_paint.set_color(text_color);
        // Draw the glyph bitmap onto the pixmap
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col] as f32 / 255.0;
                if alpha > 0.0 {
                    let y = self.char_height - metrics.bounds.height as u32 - metrics.ymin as u32;
                    mut_pixmap.fill_rect(
                        tiny_skia::Rect::from_xywh(
                            (metrics.bounds.xmin) + (col as f32),
                            y as f32 - (metrics.bounds.ymin) + (row as f32),
                            1.0,
                            1.0,
                        )
                        .unwrap(),
                        &self.skia_paint,
                        Transform::identity(),
                        None, // or Some(alpha) if you want to alpha-blend
                    );
                }
            }
        }

        self.screen_pixmap.draw_pixmap(
            (xik as u32 * self.char_width) as i32,
            (yik as u32 * self.char_height) as i32,
            mut_pixmap.as_ref(),
            &self.pixmap_paint,
            Transform::identity(),
            None,
        );
    }

    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default())
            .expect("Failed to load font");
        let mut skia_paint = Paint::default();
        skia_paint.anti_alias = false;
        // skia_paint.blend_mode = BlendMode::Difference;
        let font_size = 20.0;

        let mut pixmap_paint = PixmapPaint::default();

        pixmap_paint.quality = FilterQuality::Nearest;

        // Rasterize each character
        let (metrics, bitmap) = font.rasterize('█', font_size);
        //  let (metrics, bitmap) = font.rasterize('}', font_size);
        println!("{metrics:#?}");

        let char_width = metrics.advance_width as u32;
        let char_height = metrics.height as u32;

        let glyph_pixmap = Pixmap::new(metrics.advance_width as u32, char_height).unwrap();

        let screen_pixmap =
            Pixmap::new((char_width * width as u32), (char_height * height as u32)).unwrap();

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_size,
            font,
            screen_pixmap,

            pixmap_paint,

            char_width,
            char_height,
            glyph_pixmap,
            skia_paint,
        };
        let _ = return_struct.clear();
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
        //    println!("WTF");
        // Save to PNG file
        self.screen_pixmap.save_png("output_tiny_skia.png").unwrap();

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
        self.screen_pixmap
            .fill(rat_to_skia_color(&clear_cell.bg, false));
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
