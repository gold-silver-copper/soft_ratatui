//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::{fs, io};

use crate::colors::*;
use crate::pixmap::RgbPixmap;

use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::Modifier;

use cosmic_text::{
    Attrs, AttrsList, BufferLine, Color as CosmicColor, Family, LineEnding, LineIter, Metrics,
    Scroll, Shaping, Style, Weight,
};

use cosmic_text::{Buffer as CosmicBuffer, FontSystem, SwashCache};

pub struct SoftBackend {
    pub buffer: Buffer,
    pub cursor: bool,
    pub pos: (u16, u16),
    pub font_system: FontSystem,
    pub character_buffer: CosmicBuffer,
    pub char_width: usize,
    pub char_height: usize,
    pub const_color: CosmicColor,
    pub blink_counter: u16,
    pub blinking_fast: bool,
    pub blinking_slow: bool,
    pub swash_cache: SwashCache,
    pub rgba_pixmap: RgbPixmap,
    pub redraw_list: Vec<(u16, u16)>,
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

    pub fn draw_cell(&mut self, xik: u16, yik: u16) {
        // Prepare Pixmap to draw into

        // Draw using tiny-skia

        let rat_cell = self.buffer.cell(Position::new(xik, yik)).unwrap();

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

        let (mut fg_color, mut bg_color) = if is_reversed {
            (rat_to_rgb(&rat_bg, false), rat_to_rgb(&rat_fg, true))
        } else {
            (rat_to_rgb(&rat_fg, true), rat_to_rgb(&rat_bg, false))
        };

        if is_dim {
            (fg_color, bg_color) = (dim_rgb(fg_color), dim_rgb(bg_color));
        };

        let begin_x = xik as usize * self.char_width;
        let begin_y = yik as usize * self.char_height;
        for y in 0..self.char_height {
            for x in 0..self.char_width {
                self.rgba_pixmap.put_pixel(
                    (begin_x + x),
                    (begin_y + y),
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

        if is_slowblink {
            if self.blinking_slow {
                fg_color = bg_color.clone();
            }
        }
        if is_rapidblink {
            if self.blinking_fast {
                fg_color = bg_color.clone();
            }
        }

        let mut attrs = Attrs::new().family(Family::Monospace);
        if is_bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if is_italic {
            attrs = attrs.style(Style::Italic);
        }

        self.character_buffer.lines = vec![BufferLine::new(
            &text_symbol,
            LineEnding::None,
            AttrsList::new(&attrs),
            Shaping::Advanced,
        )];

        self.character_buffer
            .shape_until_scroll(&mut self.font_system, false);

        for run in self.character_buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0., 0.), 1.0);

                //TODO : Handle Content::Color (emojis?)

                if let Some(image) = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical_glyph.cache_key)
                {
                    //    println!("imagik {:#?}", image.data.len());
                    let x = image.placement.left;

                    let y = -image.placement.top;
                    let mut i = 0;

                    for off_y in 0..image.placement.height {
                        for off_x in 0..image.placement.width {
                            //TODO: blend base alpha?

                            let real_x = physical_glyph.x + x + off_x as i32;
                            let real_y = run.line_y as i32 + physical_glyph.y + y + off_y as i32;
                            //   println!("{}", run.line_y);
                            if real_x >= 0 && real_y >= 0 {
                                let get_x = (begin_x + real_x as usize);
                                let get_y = (begin_y + real_y as usize);

                                let put_color = blend_rgba(
                                    [fg_color[0], fg_color[1], fg_color[2], image.data[i]],
                                    [bg_color[0], bg_color[1], bg_color[2], 255],
                                );
                                self.rgba_pixmap.put_pixel(get_x, get_y, put_color);
                            }

                            i += 1;
                        }
                    }
                }

                /*self.swash_cache.with_pixels(
                    &mut self.font_system,
                    physical_glyph.cache_key,
                    self.const_color,
                    |x, y, color| {
                        let real_x = physical_glyph.x + x;
                        let real_y = run.line_y as i32 + physical_glyph.y + y;
                        if real_x >= 0 && real_y >= 0 {
                            let get_x = (begin_x as i32 + real_x) as usize;
                            let get_y = (begin_y as i32 + real_y) as usize;

                            let put_color = blend_rgba(
                                [fg_color[0], fg_color[1], fg_color[2], color.a()],
                                [bg_color[0], bg_color[1], bg_color[2], 255],
                            );
                            self.rgba_pixmap.put_pixel(get_x, get_y, put_color);
                        }
                    },
                ); */
            }
        }
    }

    pub fn new(width: u16, height: u16, font_size: i32) -> Self {
        let mut swash_cache = SwashCache::new();

        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(font_size as f32, font_size as f32);
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
        let char_width = wa.width as usize;
        let char_height = wa.height as usize;
        character_buffer.set_size(
            &mut font_system,
            Some(char_width as f32),
            Some(char_height as f32),
        );

        let const_color = CosmicColor::rgb(255, 255, 255);

        let rgba_pixmap = RgbPixmap::new(
            (char_width * width as usize),
            (char_height * height as usize),
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
            const_color,
            blink_counter: 0,
            blinking_fast: false,
            blinking_slow: false,
            redraw_list: Vec::new(),

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

    //TODO fix redraw to use redraw list
    pub fn redraw(&mut self) {
        for x in 0..self.buffer.area.width {
            for y in 0..self.buffer.area.height {
                self.draw_cell(x, y);
            }
        }
    }
    // Call this every tick/frame
    // Call this every tick/frame
    pub fn update_blinking(&mut self) {
        self.blink_counter = (self.blink_counter + 1) % 200;

        self.blinking_fast = matches!(self.blink_counter % 100, 0..=5); // fast blink: 5 ticks on, 5 off
        self.blinking_slow = matches!(self.blink_counter, 20..=25); // slow blink: ticks 20–29 only
    }
}

impl Backend for SoftBackend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.update_blinking();
        for (x, y, c) in content {
            if c.modifier.contains(Modifier::SLOW_BLINK)
                || c.modifier.contains(Modifier::RAPID_BLINK)
            {
                self.redraw_list.push((x, y));
            }
            self.buffer[(x, y)] = c.clone();
            self.draw_cell(x, y);
            //   println!("{c:#?}");
        }
        for (x, y) in self.redraw_list.clone().iter() {
            self.draw_cell(*x, *y);
        }
        // self.redraw();

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
