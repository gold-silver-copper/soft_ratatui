//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::collections::HashSet;
use std::{char, io};

use crate::colors::*;
use crate::pixmap::RgbPixmap;

use ab_glyph::{Font, FontRef, Glyph, PxScale, PxScaleFont, ScaleFont, point};

use glyph_brush_layout::{GlyphPositioner, SectionText};
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::Modifier;

/// SoftBackend is a Software rendering backend for Ratatui. It stores the generated image internally as rgb_pixmap.
pub struct SoftBackend {
    pub buffer: Buffer,
    pub cursor: bool,
    pub pos: (u16, u16),
    font: PxScaleFont<FontRef<'static>>,
    font_size: i32,

    pub char_width: usize,
    pub char_height: usize,

    pub blink_counter: u16,
    pub blinking_fast: bool,
    pub blinking_slow: bool,

    pub rgb_pixmap: RgbPixmap,
    always_redraw_list: HashSet<(u16, u16)>,
}

fn add_strikeout(text: &String) -> String {
    let strike = '\u{0336}';
    text.chars().flat_map(|c| [c, strike]).collect()
}

fn add_underline(text: &String) -> String {
    let strike = '\u{0332}';
    text.chars().flat_map(|c| [c, strike]).collect()
}

impl SoftBackend {
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
        let pix_wid = self.get_pixmap_width();
        let pix_hei = self.get_pixmap_height();
        let char = rat_cell.symbol().chars().next().unwrap();

        let glyph: Glyph = self.font.scaled_glyph(char);

        if let Some(image) = self.font.outline_glyph(glyph) {
            image.draw(|x, y, c| {
                let get_x = begin_x + x as usize;
                let get_y = begin_y + y as usize;
                if get_x < pix_wid && get_y < pix_hei && c > 0.3 {
                    self.rgb_pixmap.put_pixel(
                        get_x,
                        get_y,
                        [fg_color[0], fg_color[1], fg_color[2]],
                    );
                }
            });
        }
    }

    /// Sets a new font size for the terminal image.
    /// This will recreate the pixmap and do a full redraw. Do not run every frame.
    pub fn set_font_size(&mut self, font_size: i32) {
        let b = self.font.clone();
        let a = b.with_scale(font_size as f32);
        self.font = a;

        // Get a glyph for 'q' with a scale & position.
        let glyph: Glyph = self.font.scaled_glyph('█');
        let block = self
            .font
            .outline_glyph(glyph)
            .expect("FONT MUST HAVE BLOCK DRAWING CHARACTER ████");

        let char_width = block.px_bounds().width().round() as usize;
        let char_height = block.px_bounds().height().round() as usize;
        self.char_width = char_width;
        self.char_height = char_height;
        self.rgb_pixmap = RgbPixmap::new(
            self.char_width * self.buffer.area.width as usize,
            self.char_height * self.buffer.area.height as usize,
        );

        self.redraw();
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

    pub fn new_with_font(
        width: u16,
        height: u16,
        font_size: i32,
        font_data: &'static [u8],
    ) -> Self {
        let font: FontRef<'static> =
            FontRef::try_from_slice(font_data).expect("COULD NOT LOAD FONT");

        let scaled_font: PxScaleFont<FontRef<'static>> = PxScaleFont {
            font: font,
            scale: PxScale::from(font_size as f32),
        };

        // Get a glyph for 'q' with a scale & position.
        let glyph: Glyph = scaled_font.scaled_glyph('█');
        let block = scaled_font
            .outline_glyph(glyph)
            .expect("FONT MUST HAVE BLOCK DRAWING CHARACTER ████");

        let char_width = block.px_bounds().width().round() as usize;
        let char_height = block.px_bounds().height().round() as usize;

        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font: scaled_font,
            font_size: font_size,

            rgb_pixmap,

            char_width,
            char_height,

            blink_counter: 0,
            blinking_fast: false,
            blinking_slow: false,
            always_redraw_list: HashSet::new(),
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
        self.always_redraw_list = HashSet::new();
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

impl Backend for SoftBackend {
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
        Ok(self.pos.into())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.pos = position.into().into();
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

fn is_unicode_block_drawing(c: char) -> bool {
    let code = c as u32;

    // Block Elements: U+2580 - U+259F
    (0x2580..=0x259F).contains(&code) ||
    // Box Drawing: U+2500 - U+257F
    (0x2500..=0x257F).contains(&code) ||
    // Geometric Shapes: U+25A0 - U+25FF
    (0x25A0..=0x25FF).contains(&code) ||
    // Miscellaneous Symbols and Arrows: U+2B00 - U+2BFF
    (0x2B00..=0x2BFF).contains(&code) ||
    // Supplemental Arrows-C: U+1F800 - U+1F8FF
    (0x1F800..=0x1F8FF).contains(&code)
}
