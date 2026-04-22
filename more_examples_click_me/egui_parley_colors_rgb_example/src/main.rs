use std::time::{Duration, Instant};

use eframe::egui::{self, TextureHandle, TextureOptions, Vec2};
use palette::convert::FromColorUnclamped;
use palette::{Okhsv, Srgb};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    prelude::{Color, Stylize, Terminal},
    text::Text,
    widgets::Widget,
};
use soft_ratatui::{ParleyText, SoftBackend};

static FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/iosevka.ttf"
));

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };

    eframe::run_native(
        "soft_ratatui egui parley colors-rgb",
        options,
        Box::new(|_cc| Ok(Box::new(ParleyColorsRgbApp::default()))),
    )
}

struct ParleyColorsRgbApp {
    terminal: Terminal<SoftBackend<ParleyText>>,
    texture: Option<TextureHandle>,
    app: ColorsRgbApp,
}

impl Default for ParleyColorsRgbApp {
    fn default() -> Self {
        let backend = SoftBackend::<ParleyText>::new(80, 30, 18, FONT);
        let terminal = Terminal::new(backend).expect("failed to create terminal");
        Self {
            terminal,
            texture: None,
            app: ColorsRgbApp::default(),
        }
    }
}

impl eframe::App for ParleyColorsRgbApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |_ui| {});

        egui::Window::new("Parley colors-rgb")
            .default_pos([32.0, 32.0])
            .default_size([960.0, 720.0])
            .resizable(true)
            .movable(true)
            .show(ctx, |ui| {
                let available = ui.available_size_before_wrap();
                self.resize_terminal_to_fit(available);
                self.app.run(&mut self.terminal);
                self.upload_texture(ctx);

                if let Some(texture) = &self.texture {
                    ui.image((texture.id(), texture.size_vec2()));
                }
            });

        ctx.request_repaint();
    }
}

impl ParleyColorsRgbApp {
    fn resize_terminal_to_fit(&mut self, available: Vec2) {
        let cell_width = self.terminal.backend().char_width.max(1) as f32;
        let cell_height = self.terminal.backend().char_height.max(1) as f32;
        let columns = (available.x / cell_width).floor().max(8.0) as u16;
        let rows = (available.y / cell_height).floor().max(4.0) as u16;
        let size = self.terminal.size().expect("terminal backend size");

        if size.width != columns || size.height != rows {
            self.terminal.backend_mut().resize(columns, rows);
        }
    }

    fn upload_texture(&mut self, ctx: &egui::Context) {
        let image = egui::ColorImage::from_rgb(
            [
                self.terminal.backend().get_pixmap_width(),
                self.terminal.backend().get_pixmap_height(),
            ],
            self.terminal.backend().get_pixmap_data(),
        );

        if let Some(texture) = &mut self.texture {
            texture.set(image, TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("parley-colors-rgb", image, TextureOptions::NEAREST));
        }
    }
}

#[derive(Debug, Default)]
struct ColorsRgbApp {
    fps_widget: FpsWidget,
    colors_widget: ColorsWidget,
}

#[derive(Debug)]
struct FpsWidget {
    frame_count: usize,
    last_instant: Instant,
    fps: Option<f32>,
}

#[derive(Debug, Default)]
struct ColorsWidget {
    colors: Vec<Vec<Color>>,
    frame_count: usize,
}

impl ColorsRgbApp {
    fn run(&mut self, terminal: &mut Terminal<SoftBackend<ParleyText>>) {
        let _ = terminal.draw(|frame| frame.render_widget(self, frame.area()));
    }
}

impl Widget for &mut ColorsRgbApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use Constraint::{Length, Min};

        let [top, colors] = Layout::vertical([Length(1), Min(0)]).areas(area);
        let [title, fps] = Layout::horizontal([Min(0), Length(10)]).areas(top);

        Text::from("colors-rgb example in egui + parley. Drag or resize this window")
            .centered()
            .white()
            .render(title, buf);
        self.fps_widget.render(fps, buf);
        self.colors_widget.render(colors, buf);
    }
}

impl Default for FpsWidget {
    fn default() -> Self {
        Self {
            frame_count: 0,
            last_instant: Instant::now(),
            fps: None,
        }
    }
}

impl Widget for &mut FpsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.calculate_fps();
        if let Some(fps) = self.fps {
            Text::from(format!("{fps:>5.1} fps"))
                .white()
                .slow_blink()
                .render(area, buf);
        }
    }
}

impl FpsWidget {
    fn calculate_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_instant.elapsed();
        if elapsed > Duration::from_secs(1) && self.frame_count > 2 {
            self.fps = Some(self.frame_count as f32 / elapsed.as_secs_f32());
            self.frame_count = 0;
            self.last_instant = Instant::now();
        }
    }
}

impl Widget for &mut ColorsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.setup_colors(area);
        let colors = &self.colors;
        for (xi, x) in (area.left()..area.right()).enumerate() {
            let xi = (xi + self.frame_count) % (area.width as usize);
            for (yi, y) in (area.top()..area.bottom()).enumerate() {
                let fg = colors[yi * 2][xi];
                let bg = colors[yi * 2 + 1][xi];
                buf[Position::new(x, y)].set_char('▀').set_fg(fg).set_bg(bg);
            }
        }
        self.frame_count += 1;
    }
}

impl ColorsWidget {
    fn setup_colors(&mut self, size: Rect) {
        let Rect { width, height, .. } = size;
        let height = height as usize * 2;
        let width = width as usize;

        if width == 0 || height == 0 {
            self.colors.clear();
            return;
        }

        if self.colors.len() == height && self.colors.first().is_some_and(|row| row.len() == width)
        {
            return;
        }

        self.colors = Vec::with_capacity(height);
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let hue = x as f32 * 360.0 / width as f32;
                let value = (height - y) as f32 / height as f32;
                let saturation = Okhsv::max_saturation();
                let color = Okhsv::new(hue, saturation, value);
                let color = Srgb::<f32>::from_color_unclamped(color);
                let color: Srgb<u8> = color.into_format();
                row.push(Color::Rgb(color.red, color.green, color.blue));
            }
            self.colors.push(row);
        }
    }
}
