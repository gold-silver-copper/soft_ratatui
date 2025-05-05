//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::{fs, io};

use crate::colors::*;
use crate::pixmap::RgbPixmap;

use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::Modifier;

use cosmic_text::{Attrs, Color, Family, Metrics, Shaping, Style, Weight};

use cosmic_text::{Buffer as CosmicBuffer, FontSystem, SwashCache};

pub struct SoftBackend {
    pub buffer: Buffer,
    pub cursor: bool,
    pub pos: (u16, u16),
    pub font_system: FontSystem,
    pub character_buffer: CosmicBuffer,
    pub char_width: u32,
    pub char_height: u32,

    pub swash_cache: SwashCache,
    pub rgba_pixmap: RgbPixmap,
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
        // Prepare Pixmap to draw into

        // Draw using tiny-skia

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
            (rat_to_rgb(&rat_bg, false), rat_to_rgb(&rat_fg, true))
        } else {
            (rat_to_rgb(&rat_fg, true), rat_to_rgb(&rat_bg, false))
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

        let mut mut_buffer = self.character_buffer.borrow_with(&mut self.font_system);

        // Set and shape text
        mut_buffer.set_text(
            &text_symbol,
            &attrs,
            Shaping::Advanced, // Basic for better performance
        );
        //mut_buffer.shape_until_scroll(true);

        mut_buffer.draw(
            &mut self.swash_cache,
            rat_to_cosmic_color(&rat_fg, true),
            |x, y, w, h, color| {
                if x >= 0 && y >= 0 {
                    let [r, g, b, a] = color.as_rgba();

                    let get_x = (xik as i32 * self.char_width as i32 + x) as usize;
                    let get_y = (yik as i32 * self.char_height as i32 + y) as usize;
                    let bg_pixel = self.rgba_pixmap.get_pixel(get_x, get_y);
                    let put_color = blend_rgba(
                        [fg_color[0], fg_color[1], fg_color[2], a],
                        [bg_pixel[0], bg_pixel[1], bg_pixel[2], 255],
                    );
                    self.rgba_pixmap.put_pixel(
                        get_x, get_y, put_color, //alpha instead of fg_color 3
                    );

                    /*
                    //bg_color or bg_pixel for put_color?
                     */
                }
            },
        );

        /*   self.screen_pixmap.draw_pixmap(
            (xik as u32 * self.char_width) as i32,
            (yik as u32 * self.char_height) as i32,
            mut_pixmap.as_ref(),
            &self.pixmap_paint,
            Transform::identity(),
            None,
        ); */
    }

    pub fn new(width: u16, height: u16, font_path: &str) -> Self {
        let mut swash_cache = SwashCache::new();

        let line_height = 16;
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
        let boop = buffer.layout_runs().next().unwrap();
        let physical_glyph = boop.glyphs.iter().next().unwrap().physical((0., 0.), 1.0);

        let wa = swash_cache
            .get_image(&mut font_system, physical_glyph.cache_key)
            .clone()
            .unwrap()
            .placement;
        println!("Glyph height (bbox): {:#?}", wa);

        let mut character_buffer = CosmicBuffer::new(&mut font_system, metrics);

        // let mut character_buffer = character_buffer.borrow_with(&mut font_system);

        //  println!("Glyph height (bbox): {:#?}", boop);
        //      // Set a size for the text buffer, in pixels
        let char_width = wa.width;
        let char_height = wa.height;
        character_buffer.set_size(
            &mut font_system,
            Some(char_width as f32),
            Some(char_height as f32),
        );

        let rgba_pixmap = RgbPixmap::new(
            (char_width * width as u32) as usize,
            (char_height * height as u32) as usize,
        );

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,

            rgba_pixmap,
            character_buffer,
            char_width,
            char_height,

            swash_cache,
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
        let colorik = rat_to_rgb(&clear_cell.bg, false);

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
