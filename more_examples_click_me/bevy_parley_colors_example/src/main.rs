use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use palette::convert::FromColorUnclamped;
use palette::{Okhsv, Srgb};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    prelude::{Color, Terminal},
    text::Text,
    widgets::Widget,
};
use soft_ratatui::{ParleyText, SoftBackend};

static FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/JetBrainsMono-Regular.ttf"
));

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: bevy::window::WindowResolution::default()
                            .with_scale_factor_override(1.0),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_non_send_resource(SoftTerminal::default())
        .insert_resource(ColorsRgbApp::default())
        .add_systems(Startup, setup)
        .add_systems(Update, render_terminal)
        .run();
}

fn setup(
    mut commands: Commands,
    soft_terminal: NonSend<SoftTerminal>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn((Camera2d, Msaa::Off));

    let width = soft_terminal.backend().get_pixmap_width() as u32;
    let height = soft_terminal.backend().get_pixmap_height() as u32;
    let data = soft_terminal.backend().get_pixmap_data_as_rgba();

    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let handle = images.add(image);
    commands.spawn(Sprite::from_image(handle.clone()));
    commands.insert_resource(SoftTerminalImage(handle));
}

fn render_terminal(
    mut soft_terminal: NonSendMut<SoftTerminal>,
    mut app: ResMut<ColorsRgbApp>,
    mut images: ResMut<Assets<Image>>,
    image_handle: Res<SoftTerminalImage>,
) {
    soft_terminal
        .draw(|frame| {
            frame.render_widget(&mut *app, frame.area());
        })
        .expect("failed to render ratatui frame");

    let width = soft_terminal.backend().get_pixmap_width() as u32;
    let height = soft_terminal.backend().get_pixmap_height() as u32;
    let data = soft_terminal.backend().get_pixmap_data_as_rgba();

    let image = images
        .get_mut(&image_handle.0)
        .expect("soft terminal image not found");
    *image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
}

#[derive(Deref, DerefMut)]
struct SoftTerminal(Terminal<SoftBackend<ParleyText>>);

impl Default for SoftTerminal {
    fn default() -> Self {
        let backend = SoftBackend::<ParleyText>::new(80, 30, 20, FONT);
        Self(Terminal::new(backend).unwrap())
    }
}

#[derive(Resource)]
struct SoftTerminalImage(Handle<Image>);

#[derive(Debug, Resource, Default)]
struct ColorsRgbApp {
    fps_widget: FpsWidget,
    colors_widget: ColorsWidget,
}

#[derive(Debug)]
struct FpsWidget {
    frame_count: usize,
    last_instant: std::time::Instant,
    fps: Option<f32>,
}

#[derive(Debug, Default)]
struct ColorsWidget {
    colors: Vec<Vec<Color>>,
    frame_count: usize,
}

impl Widget for &mut ColorsRgbApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use Constraint::{Length, Min};

        let [top, colors] = area.layout(&Layout::vertical([Length(1), Min(0)]));
        let [title, fps] = top.layout(&Layout::horizontal([Min(0), Length(8)]));

        Text::from("colors-rgb example ported to Bevy + parley. Press close window to quit")
            .centered()
            .render(title, buf);
        self.fps_widget.render(fps, buf);
        self.colors_widget.render(colors, buf);
    }
}

impl Default for FpsWidget {
    fn default() -> Self {
        Self {
            frame_count: 0,
            last_instant: std::time::Instant::now(),
            fps: None,
        }
    }
}

impl Widget for &mut FpsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.calculate_fps();
        if let Some(fps) = self.fps {
            Text::from(format!("{fps:.1} fps")).render(area, buf);
        }
    }
}

impl FpsWidget {
    fn calculate_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_instant.elapsed();
        if elapsed > std::time::Duration::from_secs(1) && self.frame_count > 2 {
            self.fps = Some(self.frame_count as f32 / elapsed.as_secs_f32());
            self.frame_count = 0;
            self.last_instant = std::time::Instant::now();
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
