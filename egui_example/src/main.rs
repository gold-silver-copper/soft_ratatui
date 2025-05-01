use eframe::egui;
/// A minimal example of a Ratatui application.
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use soft_ratatui::SoftBackend;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 1000.0]),
        ..Default::default()
    };

    let my_app = MyApp::new();

    eframe::run_native(
        "Image Viewer",
        options,
        Box::new(|cc| {
            // This gives us image support:

            Ok(Box::new(my_app))
        }),
    )
}

struct MyApp {
    pub terminal: Terminal<SoftBackend>,
}

impl MyApp {
    fn new() -> Self {
        let backend = SoftBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.clear();
        Self { terminal }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // terminal.draw(draw).expect("failed to draw frame");
        self.terminal.draw(|frame| {
            let area = frame.area();
            let textik = format!("Hello egui! The window area is {}", area);
            frame.render_widget(
                Paragraph::new(textik)
                    .block(Block::new().title("Ratatui").borders(Borders::ALL))
                    .wrap(Wrap { trim: false }),
                area,
            );
        });

        let colorik = egui::ColorImage::from_rgb(
            [
                self.terminal.backend().get_pixmap_width(),
                self.terminal.backend().get_pixmap_height(),
            ],
            self.terminal.backend().get_pixmap_data(),
        );

        let texture = ctx.load_texture(
            "my-color-image", // texture ID (can be anything)
            colorik.clone(),  // your ColorImage
            Default::default(),
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image((texture.id(), texture.size_vec2()));
            });
        });
    }
}

fn draw(frame: &mut Frame) {
    let text = Text::raw("Hello World!");
    frame.render_widget(text, frame.area());
}
