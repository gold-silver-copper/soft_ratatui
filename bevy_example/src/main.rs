use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use embedded_graphics_unicodefonts::{
    mono_8x13_atlas, mono_8x13_bold_atlas, mono_8x13_italic_atlas,
};
use ratatui::{
    prelude::{Stylize, Terminal},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use soft_ratatui::SoftBackend;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<SoftTerminal>()
        .add_systems(Startup, setup)
        .add_systems(Update, ui_example_system)
        .run();
}
fn setup(
    mut commands: Commands,
    mut softatui: ResMut<SoftTerminal>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn(Camera2d);

    let width = softatui.backend().get_pixmap_width() as u32;
    let height = softatui.backend().get_pixmap_height() as u32;
    let data = softatui.backend().get_pixmap_data_as_rgba();

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
    commands.insert_resource(MyProcGenImage(handle));
}

// Render to the terminal and to egui , both are immediate mode
fn ui_example_system(
    mut softatui: ResMut<SoftTerminal>,
    mut images: ResMut<Assets<Image>>,
    my_handle: Res<MyProcGenImage>,
) {
    softatui
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

    let width = softatui.backend().get_pixmap_width() as u32;
    let height = softatui.backend().get_pixmap_height() as u32;
    let data = softatui.backend().get_pixmap_data_as_rgba();

    let image = images.get_mut(&my_handle.0).expect("Image not found");
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

// Create resource to hold the ratatui terminal
#[derive(Resource, Deref, DerefMut)]
struct SoftTerminal(Terminal<SoftBackend>);
impl Default for SoftTerminal {
    fn default() -> Self {
        let font_regular = mono_8x13_atlas();
        let font_italic = mono_8x13_italic_atlas();
        let font_bold = mono_8x13_bold_atlas();
        let backend = SoftBackend::new(100, 50, font_regular, Some(font_bold), Some(font_italic));
        //backend.set_font_size(12);
        Self(Terminal::new(backend).unwrap())
    }
}

#[derive(Resource)]
struct MyProcGenImage(Handle<Image>);
