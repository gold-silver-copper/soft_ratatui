//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::{char, io};

use crate::SoftBackend;
use crate::colors::*;
use crate::pixmap::RgbPixmap;
use bdf_parser::*;
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::Modifier;
use rustc_hash::FxHashSet;

/// SoftBackend is a Software rendering backend for Ratatui. It stores the generated image internally as rgb_pixmap.
pub struct Bdf {
    font: Font,
}

fn add_strikeout(text: &String) -> String {
    let strike = '\u{0336}';
    text.chars().flat_map(|c| [c, strike]).collect()
}

fn add_underline(text: &String) -> String {
    let strike = '\u{0332}';
    text.chars().flat_map(|c| [c, strike]).collect()
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

        let mut rat_fg = rat_cell.fg;
        let rat_bg = rat_cell.bg;
        if rat_cell.modifier.contains(Modifier::HIDDEN) {
            rat_fg = rat_bg;
        }

        let (mut fg_color, mut bg_color) = if rat_cell.modifier.contains(Modifier::REVERSED) {
            (rat_to_rgb(&rat_bg, false), rat_to_rgb(&rat_fg, true))
        } else {
            (rat_to_rgb(&rat_fg, true), rat_to_rgb(&rat_bg, false))
        };

        if rat_cell.modifier.contains(Modifier::DIM) {
            (fg_color, bg_color) = (dim_rgb(fg_color), dim_rgb(bg_color));
        };

        let begin_x = xik as usize * self.char_width;
        let begin_y = yik as usize * self.char_height;

        for y in 0..self.char_height {
            let y_pos = begin_y + y;
            let mut x_pos = begin_x;
            for _ in 0..self.char_width {
                self.rgb_pixmap.put_pixel(x_pos, y_pos, bg_color);
                x_pos += 1;
            }
        }

        let mut text_symbol: String = rat_cell.symbol().to_string();

        if rat_cell.modifier.contains(Modifier::CROSSED_OUT) {
            text_symbol = add_strikeout(&text_symbol);
        }
        if rat_cell.modifier.contains(Modifier::UNDERLINED) {
            text_symbol = add_underline(&text_symbol);
        }

        if rat_cell.modifier.contains(Modifier::SLOW_BLINK) {
            self.always_redraw_list.insert((xik, yik));
            if self.blinking_slow {
                fg_color = bg_color.clone();
            }
        }
        if rat_cell.modifier.contains(Modifier::RAPID_BLINK) {
            self.always_redraw_list.insert((xik, yik));
            if self.blinking_fast {
                fg_color = bg_color.clone();
            }
        }

        let char = rat_cell.symbol().chars().next().unwrap();

        /*      let y_iter = glyph.bounding_box.size.y - glyph.bounding_box.offset.y;
        for x in 0..glyph.bounding_box.size.x {
            for y in 0..y_iter { */

        // Replace 10 with whatever you read from the font properties (FONT_ASCENT).
        let ascent: i32 = 10;
        let descent: i32 = 3;
        let cell_h = ascent + descent; // 13

        if let Some(glyph) = self.raster_backend.font.glyphs.get(char) {
            // glyph bitmap size (top-down rows in BDF BITMAP)
            let gw = glyph.bounding_box.size.x as i32;
            let gh = glyph.bounding_box.size.y as i32;
            let off_x = glyph.bounding_box.offset.x; // BBX x offset (signed)
            let off_y = glyph.bounding_box.offset.y; // BBX y offset (signed), *lower-left corner y relative to origin*

            let base_x = begin_x as i32; // top-left x of the cell in destination
            let base_y = begin_y as i32; // top-left y of the cell in destination

            // For BDF: BITMAP rows are listed top-to-bottom. We'll assume glyph.pixel(x,y)
            // expects y=0 as the top row of the glyph bitmap. If that's not true,
            // see note below about flipping `sample_sy`.
            let y_is_top_down = true;

            // Iterate over the glyph bitmap (sx: left->right, sy: top->bottom)
            for sy in 0..gh {
                let sample_sy = if y_is_top_down {
                    sy as usize
                } else {
                    // flip if glyph.pixel expects bottom-up indexing
                    (gh - 1 - sy) as usize
                };

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

                    self.rgb_pixmap.put_pixel(
                        dst_x,
                        dst_y,
                        [fg_color[0], fg_color[1], fg_color[2]],
                    );
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

    pub fn new(width: u16, height: u16, font_size: (usize, usize), font_data: &str) -> Self {
        let bdf_font = Font::parse(font_data).expect("COULD NOT PARSE BDF FONT DATA");
        let char_width = font_size.0;
        let char_height = font_size.1;
        let a = bdf_font.metrics;
        println!("metrics {:#?}", a);
        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            cursor_pos: (0, 0),

            raster_backend: Bdf { font: bdf_font },

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
