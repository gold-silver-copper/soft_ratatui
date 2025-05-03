//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::{fs, io};

use crate::colors::*;
use crate::pixmap::RgbPixmap;
use fontdue::Font;

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color as RatColor, Modifier};

#[derive(Debug)]
pub struct SoftBackend {
    pub buffer: Buffer,
    pub cursor: bool,
    pub pos: (u16, u16),
    pub font: Font,

    pub font_size: f32,
    pub char_width: u32,
    pub char_height: u32,
    pub rgba_pixmap: RgbPixmap,

    pub ymin: i32,
}

impl SoftBackend {
    pub fn get_pixmap_data(&self) -> &[u8] {
        self.rgba_pixmap.data()
    }
    pub fn get_pixmap_width(&self) -> usize {
        self.rgba_pixmap.width()
    }
    pub fn get_pixmap_height(&self) -> usize {
        self.rgba_pixmap.height()
    }

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
        for y in 0..self.char_height {
            for x in 0..self.char_width {
                self.rgba_pixmap.put_pixel(
                    begin_x as usize + x as usize,
                    begin_y as usize + y as usize,
                    [bg_color[0], bg_color[1], bg_color[2]],
                );
            }
        }

        let shift_y = self.char_height - metrics.height as u32;
        for row in 0..self.char_height {
            for col in 0..self.char_width {
                if row < metrics.height as u32 && col < metrics.width as u32 {
                    let alpha = bitmap[row as usize * metrics.width + col as usize];
                    if alpha > 0 {
                        let get_y = (begin_y as f32 + shift_y as f32 - metrics.bounds.ymin
                            + self.ymin as f32
                            + row as f32) as u32;
                        let get_x = (begin_x as f32 + metrics.bounds.xmin + col as f32) as u32;
                        let bg_pixel = self.rgba_pixmap.get_pixel(get_x as usize, get_y as usize);
                        //bg_color or bg_pixel for put_color?
                        let put_color = blend_rgba(
                            [fg_color[0], fg_color[1], fg_color[2], alpha],
                            [bg_pixel[0], bg_pixel[1], bg_pixel[2], 255],
                        );
                        self.rgba_pixmap.put_pixel(
                            get_x as usize,
                            get_y as usize,
                            put_color, //alpha instead of fg_color 3
                        );
                    }
                }
            }
        }
    }

    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16, font_path: &str) -> Self {
        let font_data = fs::read(font_path).unwrap();

        let font = Font::from_bytes(font_data, fontdue::FontSettings::default())
            .expect("Failed to load font");
        let font_size = 16.0;

        let (metrics, bitmap) = font.rasterize('█', font_size);
        //  let (metrics, bitmap) = font.rasterize('}', font_size);
        println!("{metrics:#?}");
        let char_width = metrics.width as u32;
        let char_height = metrics.height as u32;
        let ymin = metrics.ymin;

        let rgba_pixmap = RgbPixmap::new(
            char_width as usize * width as usize,
            char_height as usize * height as usize,
        );

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font,
            font_size,

            ymin,
            rgba_pixmap,
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
        let rgba_pixmap = RgbPixmap::new(
            self.char_width as usize * width as usize,
            self.char_height as usize * height as usize,
        );
        self.rgba_pixmap = rgba_pixmap;
        self.redraw();
    }

    pub fn redraw(&mut self) {
        for x in 0..self.buffer.area.width {
            for y in 0..self.buffer.area.height {
                let c = self.buffer[(x, y)].clone();
                self.draw_cell(&c, x, y);
            }
        }
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
        let colorik = rat_to_rgba(&clear_cell.bg, false);

        self.rgba_pixmap.fill([colorik[0], colorik[1], colorik[2]]);

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
