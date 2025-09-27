use embedded_graphics_unicodefonts::{
    mono_8x13_atlas, mono_8x13_bold_atlas, mono_8x13_italic_atlas,
};
use ratatui::Terminal;
/// A minimal example of a Ratatui application.
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use soft_ratatui::{EmbeddedGraphics, SoftBackend};

fn main() {
    let font_regular = mono_8x13_atlas();
    let font_italic = mono_8x13_italic_atlas();
    let font_bold = mono_8x13_bold_atlas();
    let backend = SoftBackend::<EmbeddedGraphics>::new(
        100,
        50,
        font_regular,
        Some(font_bold),
        Some(font_italic),
    );
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
