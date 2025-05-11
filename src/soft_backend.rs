//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::collections::HashSet;
use std::io;

use crate::colors::*;
use crate::pixmap::RgbPixmap;

use cosmic_text::fontdb::Database;
use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::Modifier;

use cosmic_text::{
    Attrs, AttrsList, CacheKeyFlags, Family, LineEnding, Metrics, Shaping, Weight, Wrap,
};

use cosmic_text::{Buffer as CosmicBuffer, FontSystem, SwashCache};

/// SoftBackend is a Software rendering backend for Ratatui. It stores the generated image internally as rgb_pixmap.
pub struct SoftBackend {
    pub buffer: Buffer,
    pub cursor: bool,
    pub pos: (u16, u16),
    font_system: FontSystem,

    cosmic_buffer: CosmicBuffer,
    pub char_width: usize,
    pub char_height: usize,

    pub blink_counter: u16,
    pub blinking_fast: bool,
    pub blinking_slow: bool,
    swash_cache: SwashCache,
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
            for x in 0..self.char_width {
                self.rgb_pixmap
                    .put_pixel(begin_x + x, begin_y + y, bg_color);
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

        let mut attrs = Attrs::new().family(Family::Monospace);
        if rat_cell.modifier.contains(Modifier::BOLD) {
            attrs = attrs.weight(Weight::BOLD);
        }
        if rat_cell.modifier.contains(Modifier::ITALIC) {
            attrs = attrs.cache_key_flags(CacheKeyFlags::FAKE_ITALIC);
        }
        let mets = self.cosmic_buffer.metrics().font_size;
        let line = self.cosmic_buffer.lines.get_mut(0).unwrap();
        line.set_text(&text_symbol, LineEnding::None, AttrsList::new(&attrs));

        line.layout(&mut self.font_system, mets, None, Wrap::None, None, 1);

        for run in self.cosmic_buffer.layout_runs() {
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
                            let real_x = physical_glyph.x + x + off_x as i32;

                            let real_y = run.line_y as i32 + physical_glyph.y + y + off_y as i32;

                            if real_x >= 0 && real_y >= 0 {
                                let get_x = begin_x + real_x as usize;
                                let get_y = begin_y + real_y as usize;

                                let put_color = blend_rgba(
                                    [fg_color[0], fg_color[1], fg_color[2], image.data[i]],
                                    [bg_color[0], bg_color[1], bg_color[2], 255],
                                );
                                self.rgb_pixmap.put_pixel(get_x, get_y, put_color);
                            }

                            i += 1;
                        }
                    }
                }
            }
        }
    }

    /// Sets a new font size for the terminal image.
    /// This will recreate the pixmap and do a full redraw. Do not run every frame.
    pub fn set_font_size(&mut self, font_size: i32) {
        let metrics = Metrics::new(font_size as f32, font_size as f32);
        self.cosmic_buffer
            .set_metrics(&mut self.font_system, metrics);
        let mut buffer = CosmicBuffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        //"█\n█",
        buffer.set_text(
            "█\n█",
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(true);
        let boop = buffer.layout_runs().next().unwrap();
        let physical_glyph = boop.glyphs.iter().next().unwrap().physical((0., 0.), 1.0);

        let wa = self
            .swash_cache
            .get_image(&mut self.font_system, physical_glyph.cache_key)
            .clone()
            .unwrap()
            .placement;
        // println!("Glyph height (bbox): {:#?}", wa);

        let char_width = wa.width as usize;
        let char_height = wa.height as usize;
        self.cosmic_buffer.set_size(
            &mut self.font_system,
            Some(char_width as f32),
            Some(char_height as f32),
        );
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

    pub fn new_with_font(width: u16, height: u16, font_size: i32, font_data: &[u8]) -> Self {
        let mut swash_cache = SwashCache::new();

        let mut db = Database::new();
        // "assets/iosevka.ttf"
        db.load_font_data(font_data.to_vec());
        //  db.set_monospace_family("Fira Mono");

        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(font_size as f32, font_size as f32);

        let boopiki = font_system.db_mut();
        *boopiki = db;

        /*   for wut in boopiki.faces() {
            println!("{wut:#?}");
        } */

        let mut buffer = CosmicBuffer::new(&mut font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut font_system);
        buffer.set_text(
            "██",
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
        // println!("Glyph height (bbox): {:#?}", wa);

        let mut cosmic_buffer = CosmicBuffer::new(&mut font_system, metrics);

        let char_width = wa.width as usize;
        let char_height = wa.height as usize;
        cosmic_buffer.set_size(
            &mut font_system,
            Some(char_width as f32),
            Some(char_height as f32),
        );

        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,

            rgb_pixmap,
            cosmic_buffer,
            char_width,
            char_height,

            blink_counter: 0,
            blinking_fast: false,
            blinking_slow: false,
            always_redraw_list: HashSet::new(),

            swash_cache,
        };
        _ = return_struct.clear();
        return_struct
    }

    /// Creates a new Software Backend using provided system fonts.
    ///
    /// (new-with-system-fonts width height font-size) -> SoftBackend
    ///
    /// * width      : usize - Width of the terminal in cells
    /// * height     : usize - Height of the terminal in cells
    /// * font-size  : u32   - Font size in pixels
    ///
    /// ⚠️ Not supported on WASM/Web targets.
    ///
    /// # Examples
    /// ```rust
    /// let backend = SoftBackend::new_with_system_fonts(20, 20, 16);
    /// ```
    pub fn new_with_system_fonts(width: u16, height: u16, font_size: i32) -> Self {
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
        //  println!("Glyph height (bbox): {:#?}", wa);

        let mut cosmic_buffer = CosmicBuffer::new(&mut font_system, metrics);

        let char_width = wa.width as usize;
        let char_height = wa.height as usize;
        cosmic_buffer.set_size(
            &mut font_system,
            Some(char_width as f32),
            Some(char_height as f32),
        );

        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,

            rgb_pixmap,
            cosmic_buffer,
            char_width,
            char_height,

            blink_counter: 0,
            blinking_fast: false,
            blinking_slow: false,
            always_redraw_list: HashSet::new(),

            swash_cache,
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
            //   println!("{c:#?}");
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
