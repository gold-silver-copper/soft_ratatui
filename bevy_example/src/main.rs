use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use ratatui::{
    prelude::{Stylize, Terminal},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use soft_ratatui::SoftBackend;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<BevyTerminal<SoftBackend>>()
        .add_systems(Startup, setup)
        //Initialize the ratatui terminal
        // Systems that create Egui widgets should be run during the `CoreSet::Update` set,
        // or after the `EguiSet::BeginFrame` system (which belongs to the `CoreSet::PreUpdate` set).
        .add_systems(Update, ui_example_system)
        .run();
}
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
}

// Render to the terminal and to egui , both are immediate mode
fn ui_example_system(
    mut commands: Commands,
    mut termres: ResMut<BevyTerminal<SoftBackend>>,
    mut images: ResMut<Assets<Image>>,
) {
    termres
        .terminal
        .draw(|frame| {
            let area = frame.area();
            let textik = format!("Hello bevy! The window area is {}", area);
            frame.render_widget(
                Paragraph::new(textik)
                    .block(Block::new().title("Ratatui").borders(Borders::ALL))
                    .white()
                    .on_blue()
                    .wrap(Wrap { trim: false }),
                area,
            );
        })
        .expect("epic fail");

    let width = termres.terminal.backend().get_pixmap_width() as u32;
    let height = termres.terminal.backend().get_pixmap_height() as u32;
    let data = termres.terminal.backend().get_pixmap_data_as_rgba();

    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    let handle = images.add(image);
    commands.spawn(Sprite::from_image(handle.clone()));
    /* commands.spawn(Sprite::from_image(
        asset_server.load("branding/bevy_bird_dark.png"),
    )); */
}

// Create resource to hold the ratatui terminal
#[derive(Resource)]
struct BevyTerminal<RataguiBackend: ratatui::backend::Backend> {
    terminal: Terminal<RataguiBackend>,
}

// Implement default on the resource to initialize it
impl Default for BevyTerminal<SoftBackend> {
    fn default() -> Self {
        let backend = SoftBackend::new(20, 20, 16);
        let terminal = Terminal::new(backend).unwrap();
        BevyTerminal { terminal }
    }
}
