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

use ratatui::backend::{Backend, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style;

/// SoftBackend is a Software rendering backend for Ratatui. It stores the generated image internally as rgb_pixmap.
pub struct SoftBackend<RasterBackend> {
    pub buffer: Buffer,
    pub cursor: bool,
    pub cursor_pos: (u16, u16),
    pub char_width: usize,
    pub char_height: usize,
    pub blink_counter: u16,
    pub blinking_fast: bool,
    pub blinking_slow: bool,
    pub rgb_pixmap: RgbPixmap,
    pub always_redraw_list: FxHashSet<(u16, u16)>,
    pub raster_backend: RasterBackend,
}
