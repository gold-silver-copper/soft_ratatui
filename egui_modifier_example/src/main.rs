/// A Ratatui example that demonstrates how to use modifiers.
///
/// It will render a grid of combinations of foreground and background colors with all
/// modifiers applied to them.
///
/// This example runs with the Ratatui library code in the branch that you are currently
/// reading. See the [`latest`] branch for the code which works with the most recent Ratatui
/// release.
///
/// [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use std::{error::Error, iter::once, result};

use eframe::egui::{self, TextureHandle};
use itertools::Itertools;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
/// A minimal example of a Ratatui application.
use ratatui::{Frame, Terminal};

use soft_ratatui::SoftBackend;
static FONT_DATA: &[u8] = include_bytes!("../../assets/iosevka.ttf");
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
    pub text_ref: Option<TextureHandle>,
}

impl MyApp {
    fn new() -> Self {
        let backend = SoftBackend::new_with_system_fonts(100, 50, 16);
        let mut terminal = Terminal::new(backend).unwrap();

        Self {
            terminal,
            text_ref: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // terminal.draw(draw).expect("failed to draw frame");
        self.terminal.draw(draw).unwrap();

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
        self.text_ref = Some(texture.clone());

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.image((texture.id(), texture.size_vec2()));
            });
        });
        ctx.request_repaint();
    }
}

fn draw(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
    let [text_area, main_area] = vertical.areas(frame.area());
    frame.render_widget(
        Paragraph::new("Note: not all terminals support all modifiers")
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        text_area,
    );
    let layout = Layout::vertical([Constraint::Length(1); 50])
        .split(main_area)
        .iter()
        .flat_map(|area| {
            Layout::horizontal([Constraint::Percentage(20); 5])
                .split(*area)
                .to_vec()
        })
        .collect_vec();

    let colors = [
        Color::Black,
        Color::DarkGray,
        Color::Gray,
        Color::White,
        Color::Red,
    ];
    let all_modifiers = once(Modifier::empty())
        .chain(Modifier::all().iter())
        .collect_vec();
    let mut index = 0;
    for bg in &colors {
        for fg in &colors {
            for modifier in &all_modifiers {
                let modifier_name = format!("{modifier:11?}");
                let padding = (" ").repeat(12 - modifier_name.len());
                let paragraph = Paragraph::new(Line::from(vec![
                    modifier_name.fg(*fg).bg(*bg).add_modifier(*modifier),
                    padding.fg(*fg).bg(*bg).add_modifier(*modifier),
                    // This is a hack to work around a bug in VHS which is used for rendering the
                    // examples to gifs. The bug is that the background color of a paragraph seems
                    // to bleed into the next character.
                    ".".black().on_black(),
                ]));
                frame.render_widget(paragraph, layout[index]);
                index += 1;
            }
        }
    }
}
