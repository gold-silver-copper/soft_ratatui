use ratatui::Terminal;
/// A minimal example of a Ratatui application.
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use soft_ratatui::{Bdf, SoftBackend};
static FONT: &str = include_str!("../assets/cozette.bdf");
fn main() {
    let backend = SoftBackend::<Bdf>::new(100, 50, (6, 13), FONT, None, None);
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
