#[cfg(any(
    feature = "unicodefonts",
    feature = "bdf-parser",
    feature = "embedded-ttf",
    feature = "cosmic-text"
))]
use ratatui::prelude::Terminal;
#[cfg(any(
    feature = "unicodefonts",
    feature = "bdf-parser",
    feature = "embedded-ttf",
    feature = "cosmic-text"
))]
use ratatui::widgets::Paragraph;

#[cfg(any(
    feature = "unicodefonts",
    feature = "bdf-parser",
    feature = "embedded-ttf",
    feature = "cosmic-text"
))]
fn draw_sample<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) {
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame.render_widget(Paragraph::new("soft_ratatui"), area);
        })
        .unwrap();
}

#[cfg(feature = "unicodefonts")]
#[test]
fn unicodefonts_backend_renders_a_frame() {
    use soft_ratatui::embedded_graphics_unicodefonts::mono_8x13_atlas;
    use soft_ratatui::{EmbeddedGraphics, SoftBackend};

    let backend = SoftBackend::<EmbeddedGraphics>::new(10, 4, mono_8x13_atlas(), None, None);
    let mut terminal = Terminal::new(backend).unwrap();
    let before = terminal.backend().get_pixmap_data().to_vec();

    draw_sample(&mut terminal);

    assert_ne!(before, terminal.backend().get_pixmap_data());
}

#[cfg(feature = "bdf-parser")]
#[test]
fn bdf_backend_renders_a_frame() {
    use soft_ratatui::{Bdf, SoftBackend};

    static FONT_DATA: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cozette.bdf"));

    let backend = SoftBackend::<Bdf>::new(10, 4, (6, 13), FONT_DATA, None, None);
    let mut terminal = Terminal::new(backend).unwrap();
    let before = terminal.backend().get_pixmap_data().to_vec();

    draw_sample(&mut terminal);

    assert_ne!(before, terminal.backend().get_pixmap_data());
}

#[cfg(feature = "embedded-ttf")]
#[test]
fn embedded_ttf_backend_font_size_changes_pixel_dimensions() {
    use soft_ratatui::rusttype::Font;
    use soft_ratatui::{EmbeddedTTF, SoftBackend};

    let font = Font::try_from_bytes(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/iosevka.ttf"
    )))
    .unwrap();
    let mut backend = SoftBackend::<EmbeddedTTF>::new(8, 4, 14, font, None, None);
    let original_size = (backend.get_pixmap_width(), backend.get_pixmap_height());

    backend.set_font_size(22);

    assert_ne!(
        original_size,
        (backend.get_pixmap_width(), backend.get_pixmap_height())
    );
}

#[cfg(feature = "cosmic-text")]
#[test]
fn cosmic_text_backend_renders_a_frame() {
    use soft_ratatui::{CosmicText, SoftBackend};

    static FONT_DATA: &[u8] =
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/iosevka.ttf"));

    let backend = SoftBackend::<CosmicText>::new(10, 4, 16, FONT_DATA);
    let mut terminal = Terminal::new(backend).unwrap();
    let before = terminal.backend().get_pixmap_data().to_vec();

    draw_sample(&mut terminal);

    assert_ne!(before, terminal.backend().get_pixmap_data());
}
