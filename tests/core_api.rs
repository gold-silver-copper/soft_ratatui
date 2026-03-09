use ratatui::backend::Backend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use rustc_hash::FxHashSet;
use soft_ratatui::{RasterBackend, RgbPixmap, SoftBackend};

#[derive(Default)]
struct DummyRaster;

impl RasterBackend for DummyRaster {
    fn draw_cell(
        &mut self,
        x: u16,
        y: u16,
        _rat_cell: &Cell,
        _always_redraw_list: &mut FxHashSet<(u16, u16)>,
        _blinking_fast: bool,
        _blinking_slow: bool,
        char_width: usize,
        char_height: usize,
        rgb_pixmap: &mut RgbPixmap,
    ) {
        for py in 0..char_height {
            for px in 0..char_width {
                rgb_pixmap.put_pixel(
                    x as usize * char_width + px,
                    y as usize * char_height + py,
                    [255, 0, 0],
                );
            }
        }
    }
}

fn make_dummy_backend(width: u16, height: u16) -> SoftBackend<DummyRaster> {
    SoftBackend {
        buffer: Buffer::empty(Rect::new(0, 0, width, height)),
        cursor: false,
        cursor_pos: (0, 0),
        char_width: 2,
        char_height: 3,
        blink_counter: 0,
        blinking_fast: false,
        blinking_slow: false,
        rgb_pixmap: RgbPixmap::new(width as usize * 2, height as usize * 3),
        always_redraw_list: FxHashSet::default(),
        raster_backend: DummyRaster,
    }
}

#[test]
fn pixmap_rgba_conversion_and_transparency_are_stable() {
    let mut pixmap = RgbPixmap::new(2, 1);
    pixmap.put_pixel(0, 0, [1, 2, 3]);
    pixmap.put_pixel(1, 0, [4, 5, 6]);

    assert_eq!(pixmap.to_rgba(), vec![1, 2, 3, 255, 4, 5, 6, 255]);
    assert_eq!(
        pixmap.to_rgba_with_color_as_transparent(&(1, 2, 3)),
        vec![1, 2, 3, 0, 4, 5, 6, 255]
    );
}

#[test]
fn soft_backend_resize_updates_terminal_and_pixel_size() {
    let mut backend = make_dummy_backend(4, 3);
    backend.resize(7, 5);

    assert_eq!(backend.size().unwrap().width, 7);
    assert_eq!(backend.size().unwrap().height, 5);
    assert_eq!(backend.get_pixmap_width(), 14);
    assert_eq!(backend.get_pixmap_height(), 15);
}

#[test]
fn soft_backend_clear_fills_entire_pixmap() {
    let mut backend = make_dummy_backend(2, 2);
    backend.rgb_pixmap.fill([255, 0, 0]);

    backend.clear().unwrap();

    assert!(
        backend
            .get_pixmap_data()
            .chunks_exact(3)
            .all(|pixel| pixel == [5, 1, 121])
    );
}
