use crate::pixmap::RgbPixmap;
use ratatui::{
    buffer::{Buffer, Cell},
    prelude::Backend,
};
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

impl Backend for SoftBackend<RasterBackend> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.update_blinking();
        for (x, y, c) in content {
            self.buffer[(x, y)] = c.clone();
            self.raster_backend.draw_cell(self, x, y);
        }
        for (x, y) in self.always_redraw_list.clone().iter() {
            self.raster_backend.draw_cell(self, *x, *y);
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
        Ok(self.cursor_pos.into())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor_pos = position.into().into();
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
