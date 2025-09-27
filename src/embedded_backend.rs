//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use embedded_graphics::text::renderer::TextRenderer;
use rustc_hash::FxHashSet;
use std::io;

use crate::colors::*;
use crate::pixmap::RgbPixmap;

use embedded_graphics::Drawable;

use embedded_graphics::mono_font::{MonoFont, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::{Point, RgbColor};
use embedded_graphics::text::{Baseline, Text};

use crate::SoftBackend;
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style;
pub struct EmbeddedGraphics {
    pub font_regular: MonoFont<'static>,
    /// Bold font.
    pub font_bold: Option<MonoFont<'static>>,
    /// Italic font.
    pub font_italic: Option<MonoFont<'static>>,
}

impl SoftBackend<EmbeddedGraphics> {
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

        let mut style_builder = MonoTextStyleBuilder::new()
            .font(&self.raster_backend.font_regular)
            .text_color(Rgb888::WHITE)
            .background_color(Rgb888::BLACK);

        for modifier in rat_cell.modifier.iter() {
            style_builder = match modifier {
                style::Modifier::BOLD => match &self.raster_backend.font_bold {
                    None => style_builder,
                    Some(font) => style_builder.font(font),
                },
                style::Modifier::DIM => {
                    (rat_fg, rat_bg) = (dim_rgb(rat_fg), dim_rgb(rat_bg));
                    style_builder
                }
                style::Modifier::ITALIC => match &self.raster_backend.font_italic {
                    None => style_builder,
                    Some(font) => style_builder.font(font),
                },
                style::Modifier::UNDERLINED => style_builder.underline(),
                style::Modifier::SLOW_BLINK => {
                    self.always_redraw_list.insert((xik, yik));
                    if self.blinking_slow {
                        rat_fg = rat_bg;
                    }
                    style_builder
                }
                style::Modifier::RAPID_BLINK => {
                    self.always_redraw_list.insert((xik, yik));
                    if self.blinking_fast {
                        rat_fg = rat_bg;
                    }
                    style_builder
                }
                style::Modifier::REVERSED => {
                    (rat_bg, rat_fg) = (rat_fg, rat_bg);

                    style_builder
                }
                style::Modifier::HIDDEN => {
                    rat_fg = rat_bg;

                    style_builder
                }
                style::Modifier::CROSSED_OUT => style_builder.strikethrough(),
                _ => style_builder,
            }
        }

        style_builder = style_builder
            .text_color(Rgb888::new(rat_fg[0], rat_fg[1], rat_fg[2]))
            .background_color(Rgb888::new(rat_bg[0], rat_bg[1], rat_bg[2]));

        let begin_x = xik as usize * self.char_width;
        let begin_y = yik as usize * self.char_height;
        Text::with_baseline(
            rat_cell.symbol(),
            Point::new(begin_x as i32, begin_y as i32),
            style_builder.build(),
            embedded_graphics::text::Baseline::Top,
        )
        .draw(&mut self.rgb_pixmap)
        .unwrap();
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
        font_regular: MonoFont<'static>,
        font_bold: Option<MonoFont<'static>>,
        font_italic: Option<MonoFont<'static>>,
    ) -> Self {
        let char_width = font_regular.character_size.width as usize;
        let char_height = font_regular.character_size.height as usize;
        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            cursor_pos: (0, 0),
            raster_backend: EmbeddedGraphics {
                font_regular: font_regular,
                font_bold,
                font_italic,
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

impl Backend for SoftBackend<EmbeddedGraphics> {
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
