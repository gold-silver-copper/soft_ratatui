/// A minimal example of a Ratatui application.
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use soft_ratatui::SoftBackend;

fn main() {
    let backend = SoftBackend::new(100, 50, 16);
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

fn draw(frame: &mut Frame) {
    let text = Text::raw("Hello World!");
    frame.render_widget(text, frame.area());
}
