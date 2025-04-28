//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

use crate::colors::*;
use fontdue::Font;
use image::{ImageBuffer, Rgba, RgbaImage};
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color as RatColor, Modifier};
static FONT_DATA: &[u8] = include_bytes!("../assets/iosevka.ttf");
#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font: Font,
    image_buffer: RgbaImage,
    font_size: f32,
    char_width: u32,
    char_height: u32,
    counter: u32,
    ymin: i32,
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
        let char = rat_cell.symbol().chars().next().unwrap();

        let (metrics, bitmap) = self.font.rasterize(char, self.font_size);

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

        let (fg_color, bg_color) = if is_reversed {
            (rat_to_rgba(&rat_bg, false), rat_to_rgba(&rat_fg, true))
        } else {
            (rat_to_rgba(&rat_fg, true), rat_to_rgba(&rat_bg, false))
        };
        let begin_x = xik as u32 * self.char_width;
        let begin_y = yik as u32 * self.char_height;
        for y in begin_y..(begin_y + self.char_height) {
            for x in begin_x..(begin_x + self.char_width) {
                self.image_buffer.put_pixel(x, y as u32, Rgba(bg_color));
            }
        }

        let y = self.char_height - metrics.height as u32;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col];
                if alpha > 0 {
                    self.image_buffer.put_pixel(
                        (begin_x as f32 + metrics.bounds.xmin + col as f32) as u32,
                        (begin_y as f32 + y as f32 - metrics.bounds.ymin
                            + self.ymin as f32
                            + row as f32) as u32,
                        Rgba(fg_color),
                    );
                }
            }
        }
    }

    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let font = Font::from_bytes(FONT_DATA, fontdue::FontSettings::default())
            .expect("Failed to load font");
        let font_size = 20.0;

        let (metrics, bitmap) = font.rasterize('█', font_size);
        //  let (metrics, bitmap) = font.rasterize('}', font_size);
        println!("{metrics:#?}");
        let char_width = metrics.width as u32;
        let char_height = metrics.height as u32;
        let ymin = metrics.ymin;
        let mut image_buffer =
            RgbaImage::new(char_width * width as u32, char_height * height as u32);
        let counter = 0;

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font,
            font_size,
            image_buffer,
            ymin,
            counter,
            char_width,
            char_height,
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

        //self.counter += 1;
        //  let title = format!("junk/my_image{}.png", self.counter);
        let title = format!("my_image.png");
        self.image_buffer.save(title).unwrap();

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

        self.image_buffer = RgbaImage::from_pixel(
            self.char_width * self.buffer.area.width as u32,
            self.char_height * self.buffer.area.height as u32,
            Rgba(rat_to_rgba(&clear_cell.bg, false)),
        );
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
