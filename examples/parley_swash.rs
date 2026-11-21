use ratatui::Terminal;
use ratatui::widgets::Paragraph;
use soft_ratatui::{ParleySwash, SoftBackend};

static FONT: &[u8] = include_bytes!("../assets/iosevka.ttf");

fn main() {
    let backend = SoftBackend::<ParleySwash>::new(100, 50, 16, FONT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("soft_ratatui Parley CPU 你好"), frame.area());
        })
        .unwrap();
}
