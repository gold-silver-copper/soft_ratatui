use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ratatui::{
    prelude::{Color, Stylize, Terminal},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use soft_ratatui::{CosmicText, SoftBackend};

static FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/JetBrainsMono-Regular.ttf"
));
const TEXT: &str = "Cosmic Text CJK demo\n\
你好，世界\n\
日本語の文章を表示します\n\
한글도 함께 보여줍니다\n\
漢字かな交じり文";

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .insert_resource(SoftTerminal::default())
        .add_systems(Startup, setup)
        .add_systems(Update, render_terminal)
        .run();
}

fn setup(
    mut commands: Commands,
    soft_terminal: Res<SoftTerminal>,
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
                            .title("Cosmic Backend")
                            .title_bottom("CJK shaping + wide cells")
                            .borders(Borders::ALL),
                    )
                    .fg(Color::White)
                    .bg(Color::Blue)
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
struct SoftTerminal(Terminal<SoftBackend<CosmicText>>);

impl Default for SoftTerminal {
    fn default() -> Self {
        let backend = SoftBackend::<CosmicText>::new(48, 16, 20, FONT);
        Self(Terminal::new(backend).unwrap())
    }
}

#[derive(Resource)]
struct SoftTerminalImage(Handle<Image>);
