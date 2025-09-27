use crate::pixmap::RgbPixmap;
use ratatui::buffer::Buffer;
use rustc_hash::FxHashSet;

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
