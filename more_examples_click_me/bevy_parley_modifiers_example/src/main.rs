use std::iter::once;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use itertools::Itertools;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::{Color, Stylize, Terminal},
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
    Frame,
};
use soft_ratatui::{ParleyText, SoftBackend};

static FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/JetBrainsMono-Regular.ttf"
));
static FONT_BOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/JetBrainsMono-Bold.ttf"
));
static FONT_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/JetBrainsMono-Italic.ttf"
));
static FONT_BOLD_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/JetBrainsMono-BoldItalic.ttf"
));

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .insert_non_send_resource(SoftTerminal::default())
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
    mut images: ResMut<Assets<Image>>,
    image_handle: Res<SoftTerminalImage>,
) {
    soft_terminal.draw(draw).expect("failed to render ratatui frame");

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

fn draw(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
    let [text_area, main_area] = vertical.areas(frame.area());
    frame.render_widget(
        Paragraph::new("Parley backend modifier grid including underline")
            .style(Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
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
                let padding = " ".repeat(12 - modifier_name.len());
                let paragraph = Paragraph::new(Line::from(vec![
                    modifier_name.fg(*fg).bg(*bg).add_modifier(*modifier),
                    padding.fg(*fg).bg(*bg).add_modifier(*modifier),
                    ".".black().on_black(),
                ]));
                frame.render_widget(paragraph, layout[index]);
                index += 1;
            }
        }
    }
}

#[derive(Deref, DerefMut)]
struct SoftTerminal(Terminal<SoftBackend<ParleyText>>);

impl Default for SoftTerminal {
    fn default() -> Self {
        let backend = SoftBackend::<ParleyText>::new_with_fonts(
            100,
            52,
            18,
            FONT,
            Some(FONT_BOLD),
            Some(FONT_ITALIC),
            Some(FONT_BOLD_ITALIC),
        );
        Self(Terminal::new(backend).unwrap())
    }
}

#[derive(Resource)]
struct SoftTerminalImage(Handle<Image>);
