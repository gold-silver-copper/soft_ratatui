//! This module provides the `SoftBackend` implementation for the [`Backend`] trait.
//! It is used in the integration tests to verify the correctness of the library.

use std::io;

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
    BlendMode, Color as SkiaColor, FilterQuality, IntSize, Paint, Pixmap, PixmapMut, PixmapPaint,
    PremultipliedColorU8, Rect as SkiaRect, Transform,
};

#[derive(Debug)]
pub struct SoftBackend {
    buffer: Buffer,
    cursor: bool,
    pos: (u16, u16),
    font_system: FontSystem,

    pixmapik: Pixmap,
    glyph_width: f32,
    glyph_height: f32,
    line_height: u32,
    pixmap_paint: PixmapPaint,
    character_buffer: CosmicBuffer,
    char_width: f32,
    char_height: u32,
    pixmap: Pixmap,
    skia_paint: Paint<'static>,
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

        let mut text_color = if is_reversed {
            self.pixmap.fill(rat_to_skia_color(&rat_fg, true));

            rat_to_cosmic_color(&rat_bg, false)
        } else {
            self.pixmap.fill(rat_to_skia_color(&rat_bg, false));

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

        self.character_buffer.set_size(
            &mut self.font_system,
            Some(self.char_width),
            Some(self.char_height as f32),
        );
        // Set and shape text
        self.character_buffer.set_text(
            &mut self.font_system,
            &text_symbol,
            &attrs,
            Shaping::Advanced, // Basic for better performance
        );
        self.character_buffer
            .shape_until_scroll(&mut self.font_system, true);

        let mut color_vec: Vec<u8> = Vec::new(); //figure out how to use color_vec instead of fill_rect
                                                 //create color vec filled with background color
                                                 //manually set each pixel thru vector rgba x + y
        let mut max_x = 0;
        let mut max_y = 0;
        self.character_buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            text_color,
            |x, y, w, h, color| {
                //   println!("{x}{y}{w}{h}");
                let colores = color.as_rgba();
                color_vec.append(&mut colores.to_vec());
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }

                if let Some(rect) = SkiaRect::from_xywh(x as f32, y as f32, 1.0, 1.0) {
                    let [r, g, b, a] = color.as_rgba();
                    self.skia_paint.set_color(SkiaColor::from_rgba8(r, g, b, a));
                    /*
                    if let Some(pixel) = self.pixmap.pixel_mut(x, y) {
                        *pixel = PremultipliedColorU8::from_color(color);
                    } */
                    self.pixmap.fill_rect(
                        rect,
                        &self.skia_paint,
                        tiny_skia::Transform::identity(),
                        None,
                    );
                }
            },
        );
        println!("stats {} {} {}", color_vec.len(), max_x, max_y);
        if let Some(int_size) = IntSize::from_wh(max_x as u32 - 1, max_y as u32 - 1) {
            if let Some(test_pixmap) = Pixmap::from_vec(color_vec, int_size) {
                println!("WOOOOOOOOOO");
            }
        }

        self.pixmapik.draw_pixmap(
            (xik as f32 * self.glyph_width) as i32,
            (yik as u32 * self.line_height) as i32,
            self.pixmap.as_ref(),
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
        let line_height = 18;
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(line_height as f32 - 3.0, line_height as f32);
        let mut buffer = CosmicBuffer::new(&mut font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut font_system);
        buffer.set_text(
            "█\n█",
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(true);
        let boop = buffer.layout_runs().next().unwrap();
        let glyph_width = boop.line_w;
        let glyph_height = boop.line_y;

        let mut pixmap_paint = PixmapPaint::default();
        let mut character_buffer = CosmicBuffer::new(&mut font_system, metrics);
        // let mut character_buffer = character_buffer.borrow_with(&mut font_system);

        pixmap_paint.quality = FilterQuality::Nearest;

        //  println!("Glyph height (bbox): {:#?}", boop);
        //      // Set a size for the text buffer, in pixels
        let char_width = glyph_width + 1.00;
        let char_height = line_height;
        let mut pixmap = Pixmap::new(char_width as u32, char_height).unwrap();

        let mut pixmapik = Pixmap::new(
            (glyph_width * width as f32) as u32,
            (metrics.line_height * height as f32) as u32,
        )
        .unwrap();

        let mut return_struct = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            pos: (0, 0),
            font_system,

            pixmapik,
            glyph_width,
            glyph_height,
            line_height,
            pixmap_paint,
            character_buffer,
            char_width,
            char_height,
            pixmap,
            skia_paint,
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
        // Save to PNG file
        self.pixmapik.save_png("output_tiny_skia.png").unwrap();

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
        for x in (0..self.buffer.area.width) {
            for y in (0..self.buffer.area.height) {
                self.draw_cell(&clear_cell, x, y);
            }
        }
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

/// Convert a ratatui color to a tiny-skia SkiaColor
pub fn rat_to_skia_color(rat_col: &RatColor, is_a_fg: bool) -> SkiaColor {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                SkiaColor::from_rgba8(204, 204, 255, 255)
            } else {
                SkiaColor::from_rgba8(15, 15, 112, 255)
            }
        }
        RatColor::Black => SkiaColor::from_rgba8(0, 0, 0, 255),
        RatColor::Red => SkiaColor::from_rgba8(139, 0, 0, 255),
        RatColor::Green => SkiaColor::from_rgba8(0, 100, 0, 255),
        RatColor::Yellow => SkiaColor::from_rgba8(255, 215, 0, 255),
        RatColor::Blue => SkiaColor::from_rgba8(0, 0, 139, 255),
        RatColor::Magenta => SkiaColor::from_rgba8(99, 9, 99, 255),
        RatColor::Cyan => SkiaColor::from_rgba8(0, 0, 255, 255),
        RatColor::Gray => SkiaColor::from_rgba8(128, 128, 128, 255),
        RatColor::DarkGray => SkiaColor::from_rgba8(64, 64, 64, 255),
        RatColor::LightRed => SkiaColor::from_rgba8(255, 0, 0, 255),
        RatColor::LightGreen => SkiaColor::from_rgba8(0, 255, 0, 255),
        RatColor::LightBlue => SkiaColor::from_rgba8(173, 216, 230, 255),
        RatColor::LightYellow => SkiaColor::from_rgba8(255, 255, 224, 255),
        RatColor::LightMagenta => SkiaColor::from_rgba8(139, 0, 139, 255),
        RatColor::LightCyan => SkiaColor::from_rgba8(224, 255, 255, 255),
        RatColor::White => SkiaColor::from_rgba8(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            // You can customize this mapping, or use a fixed 256-color palette lookup
            let i = *i as u8;
            SkiaColor::from_rgba8(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => SkiaColor::from_rgba8(*r, *g, *b, 255),
    }
}

pub fn rat_to_cosmic_color(rat_col: &RatColor, is_a_fg: bool) -> CosmicColor {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                CosmicColor::rgba(204, 204, 255, 255)
            } else {
                CosmicColor::rgba(15, 15, 112, 255)
            }
        }
        RatColor::Black => CosmicColor::rgba(0, 0, 0, 255),
        RatColor::Red => CosmicColor::rgba(139, 0, 0, 255),
        RatColor::Green => CosmicColor::rgba(0, 100, 0, 255),
        RatColor::Yellow => CosmicColor::rgba(255, 215, 0, 255),
        RatColor::Blue => CosmicColor::rgba(0, 0, 139, 255),
        RatColor::Magenta => CosmicColor::rgba(99, 9, 99, 255),
        RatColor::Cyan => CosmicColor::rgba(0, 0, 255, 255),
        RatColor::Gray => CosmicColor::rgba(128, 128, 128, 255),
        RatColor::DarkGray => CosmicColor::rgba(64, 64, 64, 255),
        RatColor::LightRed => CosmicColor::rgba(255, 0, 0, 255),
        RatColor::LightGreen => CosmicColor::rgba(0, 255, 0, 255),
        RatColor::LightBlue => CosmicColor::rgba(173, 216, 230, 255),
        RatColor::LightYellow => CosmicColor::rgba(255, 255, 224, 255),
        RatColor::LightMagenta => CosmicColor::rgba(139, 0, 139, 255),
        RatColor::LightCyan => CosmicColor::rgba(224, 255, 255, 255),
        RatColor::White => CosmicColor::rgba(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            let i = *i as u8;
            CosmicColor::rgba(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => CosmicColor::rgba(*r, *g, *b, 255),
    }
}
