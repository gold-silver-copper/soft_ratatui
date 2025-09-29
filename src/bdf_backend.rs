//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::{char, io};

use crate::SoftBackend;
use crate::colors::*;
use crate::pixmap::RgbPixmap;
use copper_bdf_parser::*;
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style;

use rustc_hash::FxHashSet;

/// SoftBackend is a Software rendering backend for Ratatui. It stores the generated image internally as rgb_pixmap.
pub struct Bdf {
    font_regular: Font,
    font_italic: Option<Font>,
    font_bold: Option<Font>,
}

impl SoftBackend<Bdf> {
    /// Retuns the raw rgb data of the pixmap as a flat array
    pub fn get_pixmap_data(&self) -> &[u8] {
        self.rgb_pixmap.data()
    }
    /// Retuns the pixmap in rgba format as a flat vector
    pub fn get_pixmap_data_as_rgba(&self) -> Vec<u8> {
        self.rgb_pixmap.to_rgba()
    }
    /// Returns the width of the pixmap in pixels
    pub fn get_pixmap_width(&self) -> usize {
        self.rgb_pixmap.width()
    }
    /// Returns the height of the pixmap in pixels
    pub fn get_pixmap_height(&self) -> usize {
        self.rgb_pixmap.height()
    }

    fn draw_cell(&mut self, xik: u16, yik: u16) {
        let rat_cell = self.buffer.cell(Position::new(xik, yik)).unwrap();

        let mut rat_fg = rat_to_rgb(&rat_cell.fg, true);
        let mut rat_bg = rat_to_rgb(&rat_cell.bg, false);
        let mut font_to_use = &self.raster_backend.font_regular;
        let mut underline = false;
        let mut crossed_out = false;
        for modifier in rat_cell.modifier.iter() {
            match modifier {
                style::Modifier::BOLD => match &self.raster_backend.font_bold {
                    None => {}
                    Some(font) => font_to_use = font,
                },
                style::Modifier::DIM => {
                    (rat_fg, rat_bg) = (dim_rgb(rat_fg), dim_rgb(rat_bg));
                }
                style::Modifier::ITALIC => match &self.raster_backend.font_italic {
                    None => {}
                    Some(font) => font_to_use = font,
                },
                style::Modifier::UNDERLINED => underline = true,
                style::Modifier::SLOW_BLINK => {
                    self.always_redraw_list.insert((xik, yik));
                    if self.blinking_slow {
                        rat_fg = rat_bg;
                    }
                }
                style::Modifier::RAPID_BLINK => {
                    self.always_redraw_list.insert((xik, yik));
                    if self.blinking_fast {
                        rat_fg = rat_bg;
                    }
                }
                style::Modifier::REVERSED => {
                    (rat_bg, rat_fg) = (rat_fg, rat_bg);
                }
                style::Modifier::HIDDEN => {
                    rat_fg = rat_bg;
                }
                style::Modifier::CROSSED_OUT => crossed_out = true,
                _ => {}
            }
        }
        let begin_x = xik as usize * self.char_width;
        let begin_y = yik as usize * self.char_height;

        for y in 0..self.char_height {
            let y_pos = begin_y + y;
            let mut x_pos = begin_x;
            for _ in 0..self.char_width {
                self.rgb_pixmap.put_pixel(x_pos, y_pos, rat_bg);
                x_pos += 1;
            }
        }

        if underline {
            let y_pos = begin_y + self.char_height - 1;
            let mut x_pos = begin_x;
            for _ in 0..self.char_width {
                self.rgb_pixmap.put_pixel(x_pos, y_pos, rat_fg);
                x_pos += 1;
            }
        }
        if crossed_out {
            let y_pos = begin_y + self.char_height / 2;
            let mut x_pos = begin_x;
            for _ in 0..self.char_width {
                self.rgb_pixmap.put_pixel(x_pos, y_pos, rat_fg);
                x_pos += 1;
            }
        }

        let char = rat_cell.symbol().chars().next().unwrap();

        let ascent = font_to_use.metrics.ascent as i32;

        if let Some(glyph) = font_to_use.glyphs.get(char) {
            // glyph bitmap size (top-down rows in BDF BITMAP)
            let gw = glyph.bounding_box.size.x;
            let gh = glyph.bounding_box.size.y;
            let off_x = glyph.bounding_box.offset.x; // BBX x offset (signed)
            let off_y = glyph.bounding_box.offset.y; // BBX y offset (signed), *lower-left corner y relative to origin*

            let base_x = begin_x as i32; // top-left x of the cell in destination
            let base_y = begin_y as i32; // top-left y of the cell in destination

            // Iterate over the glyph bitmap (sx: left->right, sy: top->bottom)
            for sy in 0..gh {
                let sample_sy = sy as usize;

                for sx in 0..gw {
                    let sample_sx = sx as usize;

                    // only render pixels that are actually set in the glyph
                    if !matches!(glyph.pixel(sample_sx, sample_sy), Some(true)) {
                        continue;
                    }

                    // Map the glyph bitmap row (sy) to a y coordinate relative to the baseline:
                    // BDF: the lower-left corner of the bitmap sits at y = off_y (relative to origin/baseline).
                    // The glyph top row (sy = 0) has relative y:
                    //    y_rel = off_y + (gh - 1 - sy)
                    // We want destination (top-down) row index:
                    //    dst_y = base_y + (ascent - 1) - y_rel
                    //
                    // Simplified algebra gives:
                    //    dst_y = base_y + ascent - off_y - gh + sy
                    //
                    // (This places sy so that a glyph whose lower-left is on the baseline (off_y = 0)
                    //  will end with its bottom row on the baseline row: dst_y == base_y + ascent - 1.)
                    let dst_x_i32 = base_x + sx + off_x;
                    let dst_y_i32 = base_y + ascent - off_y - gh + sy;

                    // signed bounds check before casting to usize
                    if dst_x_i32 < 0 || dst_y_i32 < 0 {
                        continue;
                    }
                    let dst_x = dst_x_i32 as usize;
                    let dst_y = dst_y_i32 as usize;
                    // signed bounds check before casting to usize
                    if dst_x < self.rgb_pixmap.width && dst_y < self.rgb_pixmap.height {
                        self.rgb_pixmap.put_pixel(dst_x, dst_y, rat_fg);
                    }
                }
            }
        }
    }

    /// Creates a new Software Backend with the given font data.
    ///
    /// (new-with-font width height font-size font-data) -> SoftBackend
    ///
    /// * width      : usize - Width of the terminal in cells
    /// * height     : usize - Height of the terminal in cells
    /// * font-size  : u32   - Font size in pixels
    /// * font-data  : &[u8] - Byte slice of the font (e.g., included with `include_bytes!`)
    ///
    /// # Examples
    /// ```rust
    /// static FONT_DATA: &[u8] = include_bytes!("../../assets/iosevka.ttf");
    /// let backend = SoftBackend::new_with_font(20, 20, 16, FONT_DATA);
    /// ```

    pub fn new(
        width: u16,
        height: u16,
        font_size: (usize, usize),
        font_regular: &str,
        font_bold: Option<&str>,
        font_italic: Option<&str>,
    ) -> Self {
        let bdf_font_regular = Font::parse(font_regular).expect("COULD NOT PARSE BDF FONT DATA");
        let char_width = font_size.0;
        let char_height = font_size.1;

        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let bdf_font_italic = match font_italic {
            Some(x) => Some(Font::parse(x).expect("INVALID ITALIC FONT")),
            _ => None,
        };
        let bdf_font_bold = match font_bold {
            Some(x) => Some(Font::parse(x).expect("INVALID BOLD FONT")),
            _ => None,
        };

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            cursor_pos: (0, 0),

            raster_backend: Bdf {
                font_regular: bdf_font_regular,
                font_italic: bdf_font_italic,
                font_bold: bdf_font_bold,
            },

            rgb_pixmap,

            char_width,
            char_height,

            blink_counter: 0,
            blinking_fast: false,
            blinking_slow: false,
            always_redraw_list: FxHashSet::default(),
        };
        _ = return_struct.clear();
        return_struct
    }

    /// Returns a reference to the internal buffer of the `SoftBackend`.
    pub const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Resizes the `SoftBackend` to the specified width and height.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.buffer.resize(Rect::new(0, 0, width, height));
        let rgb_pixmap = RgbPixmap::new(
            self.char_width as usize * width as usize,
            self.char_height as usize * height as usize,
        );
        self.rgb_pixmap = rgb_pixmap;
        self.redraw();
    }

    /// Redraws the pixmap
    pub fn redraw(&mut self) {
        self.always_redraw_list = FxHashSet::default();
        for x in 0..self.buffer.area.width {
            for y in 0..self.buffer.area.height {
                self.draw_cell(x, y);
            }
        }
    }

    fn update_blinking(&mut self) {
        self.blink_counter = (self.blink_counter + 1) % 200;

        self.blinking_fast = matches!(self.blink_counter % 100, 0..=5);
        self.blinking_slow = matches!(self.blink_counter, 20..=25);
    }
}

impl Backend for SoftBackend<Bdf> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.update_blinking();
        for (x, y, c) in content {
            self.buffer[(x, y)] = c.clone();
            self.draw_cell(x, y);
        }
        for (x, y) in self.always_redraw_list.clone().iter() {
            self.draw_cell(*x, *y);
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
        Ok(self.cursor_pos.into())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor_pos = position.into().into();
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.buffer.reset();
        let clear_cell = Cell::EMPTY;
        let colorik = rat_to_rgb(&clear_cell.bg, false);

        self.rgb_pixmap.fill([colorik[0], colorik[1], colorik[2]]);

        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.buffer.area.as_size())
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let window_pixels = Size {
            width: self.get_pixmap_width() as u16,
            height: self.get_pixmap_height() as u16,
        };
        Ok(WindowSize {
            columns_rows: self.buffer.area.as_size(),
            pixels: window_pixels,
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
