//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

use crate::colors::*;
use cosmic_text::Style;
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color as RatColor, Modifier};

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight,
};
use tiny_skia::{
    BlendMode, Color as SkiaColor, ColorU8 as SkiaColorU8, FilterQuality, IntSize, Paint, Pixmap,
    PixmapMut, PixmapPaint, PremultipliedColorU8, Rect as SkiaRect, Transform,
};

#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font_system: FontSystem,

    screen_pixmap: Pixmap,

    pixmap_paint: PixmapPaint,
    character_buffer: CosmicBuffer,
    char_width: u32,
    char_height: u32,
    glyph_pixmap: Pixmap,
    skia_paint: Paint<'static>,
    swash_cache: SwashCache,
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

        let mut mut_pixmap = self.glyph_pixmap.as_mut();

        let mut text_color = if is_reversed {
            mut_pixmap.fill(rat_to_skia_color(&rat_fg, true));

            rat_to_cosmic_color(&rat_bg, false)
        } else {
            mut_pixmap.fill(rat_to_skia_color(&rat_bg, false));

            rat_to_cosmic_color(&rat_fg, true)
        };

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

            let [r, g, b, a] = color.as_rgba();
            self.skia_paint.set_color(SkiaColor::from_rgba8(r, g, b, a));

            mut_pixmap.fill_rect(
                SkiaRect::from_xywh(x as f32, y as f32, w as f32, h as f32).unwrap(),
                &self.skia_paint,
                Transform::identity(),
                None,
            );
        });

        self.screen_pixmap.draw_pixmap(
            (xik as u32 * self.char_width) as i32,
            (yik as u32 * self.char_height) as i32,
            mut_pixmap.as_ref(),
            &self.pixmap_paint,
            Transform::identity(),
            None,
        );
    }

    /// Creates a new `SoftBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let mut swash_cache = SwashCache::new();
        let mut skia_paint = Paint::default();
        skia_paint.anti_alias = false;
        // skia_paint.blend_mode = BlendMode::Difference;
        let font_size = 14;
        let line_height = 17.0;
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(font_size as f32, line_height as f32);
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
        println!("physical glyph: {:#?}", physical_glyph);
        let wa = swash_cache
            .get_image(&mut font_system, physical_glyph.cache_key)
            .clone()
            .unwrap()
            .placement;
        println!("Glyph height (bbox): {:#?}", wa);

        let mut pixmap_paint = PixmapPaint::default();
        let mut character_buffer = CosmicBuffer::new(&mut font_system, metrics);

        pixmap_paint.quality = FilterQuality::Nearest;

        let char_width = wa.width;
        let char_height = wa.height;
        character_buffer.set_size(
            &mut font_system,
            Some(char_width as f32),
            Some(char_height as f32),
        );
        let mut glyph_pixmap = Pixmap::new(char_width, char_height).unwrap();

        let mut screen_pixmap =
            Pixmap::new((char_width * width as u32), (char_height * height as u32)).unwrap();

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,

            screen_pixmap,

            pixmap_paint,
            character_buffer,
            char_width,
            char_height,
            glyph_pixmap,
            skia_paint,
            swash_cache,
        };
        let _ = return_struct.clear();
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
            //   self.draw_cell(&c, x, y);
            //   println!("{c:#?}");
        }
        self.draw_buffer();
        // Save to PNG file
        self.screen_pixmap.save_png("output_tiny_skia.png").unwrap();

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
        self.screen_pixmap
            .fill(rat_to_skia_color(&clear_cell.bg, false));
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

impl SoftBackend {
    pub fn draw_buffer(&mut self) {
        let clear_cell = Cell::EMPTY;
        self.screen_pixmap
            .fill(rat_to_skia_color(&clear_cell.bg, false));
        let buffer_width = self.buffer.area.width;
        let buffer_height = self.buffer.area.height;
        let attrs = Attrs::new().family(Family::Monospace);

        println!("attrs: {attrs:#?}");

        self.character_buffer.set_size(
            &mut self.font_system,
            Some(self.char_width as f32 * buffer_width as f32),
            Some(self.char_height as f32 * buffer_height as f32),
        );

        let mut text_vec = Vec::new();

        for y in 0..buffer_height {
            for x in 0..buffer_width {
                let mut cell_attrs = attrs.clone();
                let rat_cell = self.buffer.cell((x, y)).unwrap();
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

                let mut text_color = if is_reversed {
                    rat_to_cosmic_color(&rat_bg, false)
                } else {
                    rat_to_cosmic_color(&rat_fg, true)
                };

                cell_attrs = cell_attrs.color(text_color);

                text_vec.push((rat_cell.symbol(), cell_attrs));
            }
            if y < buffer_height - 1 {
                text_vec.push(("\n", attrs.clone()));
            }
        }
        //  println!("{text_vec:#?}");
        self.character_buffer.set_rich_text(
            &mut self.font_system,
            text_vec,
            &attrs,
            Shaping::Advanced,
            None,
        );
        self.character_buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            CosmicColor::rgb(200, 200, 100),
            |x, y, w, h, color| {
                // println!("{x}{y}{w}{h}");
                if let Some(rect) = SkiaRect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
                    let [r, g, b, a] = color.as_rgba();
                    self.skia_paint.set_color(SkiaColor::from_rgba8(r, g, b, a));

                    self.screen_pixmap.fill_rect(
                        rect,
                        &self.skia_paint,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            },
        );
    }
}
