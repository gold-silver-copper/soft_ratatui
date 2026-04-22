use ratatui::Terminal;
use ratatui::widgets::Paragraph;
use soft_ratatui::{ParleyText, SoftBackend};

static FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/JetBrainsMono-Regular.ttf"
));

fn main() {
    let backend = SoftBackend::<ParleyText>::new(100, 50, 16, FONT);
    let mut terminal = Terminal::new(backend).unwrap();
    let _ = terminal.draw(|frame| {
        frame.render_widget(Paragraph::new("soft_ratatui parley 你好"), frame.area());
    });
}
