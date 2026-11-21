#![cfg(feature = "embedded-ttf")]

use ratatui::{Terminal, widgets::Paragraph};
use rusttype::Font;
use soft_ratatui::{EmbeddedTTF, SoftBackend};

#[test]
fn adjacent_terminal_art_cells_join() {
    let font = Font::try_from_bytes(include_bytes!("../assets/iosevka.ttf")).unwrap();
    let backend = SoftBackend::<EmbeddedTTF>::new(8, 3, 18, font, None, None);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("Copper\n────────\n████████"), frame.area());
        })
        .unwrap();
    let backend = terminal.backend();
    let image = &backend.rgb_pixmap;
    let w = image.width / 8;
    let h = image.height / 3;
    if let Some(path) = std::env::var_os("TTF_CAPTURE") {
        let mut ppm = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
        ppm.extend_from_slice(&image.data);
        std::fs::write(path, ppm).unwrap();
    }
    for row in 1..=2 {
        assert!(
            (row * h..(row + 1) * h).any(|y| {
                let interior = image.get_pixel(w + w / 2, y);
                interior[0] > 50 && (w..7 * w).all(|x| image.get_pixel(x, y) == interior)
            }),
            "visible seam in row {row}"
        );
    }
}
