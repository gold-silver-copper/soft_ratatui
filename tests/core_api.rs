use ratatui::backend::Backend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use rustc_hash::FxHashSet;
use soft_ratatui::{BlinkConfig, CursorConfig, CursorStyle, RasterBackend, RgbPixmap, SoftBackend};

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
        cursor_config: CursorConfig::default(),
        char_width: 2,
        char_height: 3,
        frame_count: 0,
        blink_config: BlinkConfig::default(),
        rgb_pixmap: RgbPixmap::new(width as usize * 2, height as usize * 3),
        always_redraw_list: FxHashSet::default(),
        raster_backend: DummyRaster,
        rendered_cursor: None,
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

#[test]
fn flush_draws_and_hides_the_inverse_cursor_overlay() {
    let mut backend = make_dummy_backend(2, 1);
    let cell = Cell::new("X");
    backend.draw([(0, 0, &cell)].into_iter()).unwrap();

    backend.show_cursor().unwrap();
    backend.set_cursor_position((0, 0)).unwrap();
    backend.flush().unwrap();

    assert_eq!(backend.rgb_pixmap.get_pixel(0, 0), [0, 255, 255]);

    backend.hide_cursor().unwrap();
    backend.flush().unwrap();

    assert_eq!(backend.rgb_pixmap.get_pixel(0, 0), [255, 0, 0]);
}

#[test]
fn flush_restores_the_previous_cursor_cell_when_the_cursor_moves() {
    let mut backend = make_dummy_backend(2, 1);
    let cell = Cell::new("X");
    backend
        .draw([(0, 0, &cell), (1, 0, &cell)].into_iter())
        .unwrap();

    backend.show_cursor().unwrap();
    backend.set_cursor_position((0, 0)).unwrap();
    backend.flush().unwrap();

    backend.set_cursor_position((1, 0)).unwrap();
    backend.flush().unwrap();

    assert_eq!(backend.rgb_pixmap.get_pixel(0, 0), [255, 0, 0]);
    assert_eq!(backend.rgb_pixmap.get_pixel(2, 0), [0, 255, 255]);
}

#[test]
fn flush_supports_colored_non_inverse_cursor_styles() {
    let mut backend = make_dummy_backend(1, 1);
    let cell = Cell::new("X");
    backend.draw([(0, 0, &cell)].into_iter()).unwrap();

    backend.cursor_config = CursorConfig {
        style: CursorStyle::Underline,
        blink: false,
        color: [1, 2, 3],
    };
    backend.show_cursor().unwrap();
    backend.set_cursor_position((0, 0)).unwrap();
    backend.flush().unwrap();

    assert_eq!(backend.rgb_pixmap.get_pixel(0, 0), [255, 0, 0]);
    assert_eq!(backend.rgb_pixmap.get_pixel(0, 2), [1, 2, 3]);
}

#[test]
fn blink_config_tick_updates_visibility() {
    let mut blink = BlinkConfig::default();

    assert!(!blink.tick(1));
    assert!(!blink.slow.is_hidden());
    assert!(!blink.fast.is_hidden());

    assert!(blink.tick(19));
    assert!(blink.fast.is_hidden());
    assert!(!blink.slow.is_hidden());

    assert!(blink.tick(52));
    assert!(blink.slow.is_hidden());
}

#[test]
fn cursor_blink_uses_the_configured_slow_timing() {
    let mut backend = make_dummy_backend(1, 1);
    let cell = Cell::new("X");
    backend.draw([(0, 0, &cell)].into_iter()).unwrap();
    backend.show_cursor().unwrap();
    backend.set_cursor_position((0, 0)).unwrap();
    let mut blink_config = BlinkConfig::default();
    blink_config.fps = 30;
    blink_config.slow.blinks_per_sec = 1;
    blink_config.slow.duty_percent = 100;
    backend.blink_config = blink_config;

    backend.draw(std::iter::empty()).unwrap();
    backend.flush().unwrap();

    assert_eq!(backend.rgb_pixmap.get_pixel(0, 0), [255, 0, 0]);
}
