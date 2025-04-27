//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

use crate::colors::*;
use cosmic_text::Style;
use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight,
};
use image::{ImageBuffer, Rgb, RgbImage};
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color as RatColor, Modifier};

#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font_system: FontSystem,
    image_buffer: RgbImage,

    character_buffer: CosmicBuffer,
    char_width: u32,
    char_height: u32,

    swash_cache: SwashCache,
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

        let (text_color, bg_color) = if is_reversed {
            (
                rat_to_cosmic_color(&rat_bg, false),
                rat_to_rgb(&rat_fg, true),
            )
        } else {
            (
                rat_to_cosmic_color(&rat_fg, true),
                rat_to_rgb(&rat_bg, false),
            )
        };
        let begin_x = xik as u32 * self.char_width;
        let begin_y = yik as u32 * self.char_height;
        for y in begin_y..(begin_y + self.char_height) {
            for x in begin_x..(begin_x + self.char_width) {
                self.image_buffer.put_pixel(x, y, Rgb(bg_color));
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

        mut_buffer.draw(&mut self.swash_cache, text_color, |x, y, w, h, color| {
            // println!("{x}{y}{w}{h}");

            if x > 0 && y > 0 {
                let [r, g, b, a] = color.as_rgba();
                self.image_buffer.put_pixel(
                    xik as u32 * self.char_width + x as u32,
                    yik as u32 * self.char_height + y as u32,
                    Rgb([r, g, b]),
                );
            }
        });

        /*   self.screen_pixmap.draw_pixmap(
            (xik as u32 * self.char_width) as i32,
            (yik as u32 * self.char_height) as i32,
            mut_pixmap.as_ref(),
            &self.pixmap_paint,
            Transform::identity(),
            None,
        ); */
    }

    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let mut swash_cache = SwashCache::new();

        // skia_paint.blend_mode = BlendMode::Difference;
        let line_height = 15;
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
        let mut image_buffer =
            RgbImage::new(char_width * width as u32, char_height * height as u32);

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,
            image_buffer,
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
        //  self.draw_buffer();

        // Save to PNG file

        self.image_buffer.save("my_image.png").unwrap();

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

        self.image_buffer = RgbImage::from_pixel(
            self.char_width * self.buffer.area.width as u32,
            self.char_height * self.buffer.area.height as u32,
            Rgb(rat_to_rgb(&clear_cell.bg, false)),
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
