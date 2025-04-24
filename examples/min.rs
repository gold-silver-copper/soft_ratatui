/// A minimal example of a Ratatui application.
use ratatui::text::Text;
use ratatui::{Frame, Terminal};
use soft_ratatui::SoftBackend;

fn main() {
    let backend = SoftBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(draw).expect("failed to draw frame");
}

fn draw(frame: &mut Frame) {
    let text = Text::raw("Hello World!");
    frame.render_widget(text, frame.area());
}
