/// A minimal example of a Ratatui application.
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use soft_ratatui::SoftBackend;

fn main() {
    let backend = SoftBackend::new_with_system_fonts(100, 50, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.clear();

    // terminal.draw(draw).expect("failed to draw frame");
    terminal.draw(|frame| {
        let area = frame.area();
        let textik = format!("Hello bevy! The window area is {}", area);
        frame.render_widget(
            Paragraph::new(textik)
                .block(Block::new().title("Ratatui").borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
            area,
        );
    });
}
