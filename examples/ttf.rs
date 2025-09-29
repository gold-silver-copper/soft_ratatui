use embedded_graphics_unicodefonts::{
    mono_8x13_atlas, mono_8x13_bold_atlas, mono_8x13_italic_atlas,
};
use ratatui::Terminal;
/// A minimal example of a Ratatui application.
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rusttype::Font;
use soft_ratatui::{EmbeddedGraphics, EmbeddedTTF, SoftBackend};

fn main() {
    let font_regular = Font::try_from_bytes(include_bytes!("../assets/iosevka.ttf")).unwrap();
    let backend = SoftBackend::<EmbeddedTTF>::new(100, 50, 16, font_regular, None, None);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.clear();

    terminal.draw(|frame| {
        let area = frame.area();
        let textik = format!("Hello soft! The window area is {}", area);
        frame.render_widget(
            Paragraph::new(textik)
                .block(Block::new().title("Ratatui").borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
            area,
        );
    });
}
