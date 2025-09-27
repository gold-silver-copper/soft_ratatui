use ratatui::Terminal;
/// A minimal example of a Ratatui application.
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use soft_ratatui::{CosmicText, SoftBackend};
static FONT: &[u8] = include_bytes!("../assets/fm.ttf");
fn main() {
    let backend = SoftBackend::<CosmicText>::new(100, 50, 16, FONT);
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
