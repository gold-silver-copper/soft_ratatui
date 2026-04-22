use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ratatui::{
    prelude::{Color, Stylize, Terminal},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use soft_ratatui::{Bdf, SoftBackend};

static FONT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/cozette.bdf"
));
const TEXT: &str = "BDF Cozette CJK demo\n\
ASCII: hello from cozette\n\
CJK sample: 你好世界 日本語 한글\n\
This example shows whatever the Cozette BDF can render.";

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .init_resource::<SoftTerminal>()
        .add_systems(Startup, setup)
        .add_systems(Update, render_terminal)
        .run();
}

fn setup(
    mut commands: Commands,
    soft_terminal: Res<SoftTerminal>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn(Camera2d);

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
    mut soft_terminal: ResMut<SoftTerminal>,
    mut images: ResMut<Assets<Image>>,
    image_handle: Res<SoftTerminalImage>,
) {
    soft_terminal
        .draw(|frame| {
            frame.render_widget(
                Paragraph::new(TEXT)
                    .block(
                        Block::new()
                            .title("BDF + Cozette")
                            .title_bottom("CJK text sample")
                            .borders(Borders::ALL),
                    )
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .wrap(Wrap { trim: false }),
                frame.area(),
            );
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

#[derive(Resource, Deref, DerefMut)]
struct SoftTerminal(Terminal<SoftBackend<Bdf>>);

impl Default for SoftTerminal {
    fn default() -> Self {
        let backend = SoftBackend::<Bdf>::new(48, 16, (6, 13), FONT, None, None);
        Self(Terminal::new(backend).unwrap())
    }
}

#[derive(Resource)]
struct SoftTerminalImage(Handle<Image>);
