//! Shows how to render UI to a texture. Useful for displaying UI in 3D space.

use std::f32::consts::PI;

use bevy::{
    color::palettes::css::GOLD,
    prelude::*,
    render::{
        camera::RenderTarget,
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
};
use ratatui::{
    Frame,
    prelude::{Stylize, Terminal},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use soft_ratatui::SoftBackend;
static FONT_DATA: &[u8] = include_bytes!("../../assets/iosevka.ttf");

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<SoftTerminal>()
        .add_systems(Startup, setup)
        .add_systems(Update, rotator_system)
        .add_systems(Update, computer_test)
        .run();
}

// Marks the cube, to which the UI texture is applied.
#[derive(Component)]
struct Cube;
#[derive(Resource)]
struct MyProcGenMaterial(Handle<StandardMaterial>);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut softatui: ResMut<SoftTerminal>,
    mut images: ResMut<Assets<Image>>,
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

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    // You need to set these texture usage flags in order to use the image as a render target
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;

    let image_handle = images.add(image);

    // Light
    commands.spawn(DirectionalLight::default());

    let texture_camera = commands
        .spawn((
            Camera2d,
            Camera {
                target: RenderTarget::Image(image_handle.clone().into()),
                ..default()
            },
        ))
        .id();

    commands
        .spawn((
            Node {
                // Cover the whole image
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(GOLD.into()),
            UiTargetCamera(texture_camera),
        ))
        .with_children(|parent| {
            parent.spawn(ImageNode::new(image_handle.clone()));
        });

    let cube_size = 4.0;
    let cube_handle = meshes.add(Cuboid::new(cube_size, cube_size, cube_size));

    // This material has the texture that has been rendered.
    let material_handle = materials.add(StandardMaterial {
        base_color_texture: Some(image_handle.clone()),
        reflectance: 0.02,
        unlit: false,

        ..default()
    });
    commands.insert_resource(MyProcGenMaterial(material_handle.clone()));

    // Cube with material containing the rendered UI texture.
    commands.spawn((
        Mesh3d(cube_handle),
        MeshMaterial3d(material_handle),
        Transform::from_xyz(0.0, 0.0, 1.5).with_rotation(Quat::from_rotation_x(-PI / 5.0)),
        Cube,
    ));

    // The main pass camera.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

const ROTATION_SPEED: f32 = 0.5;

fn rotator_system(time: Res<Time>, mut query: Query<&mut Transform, With<Cube>>) {
    for mut transform in &mut query {
        transform.rotate_x(1.0 * time.delta_secs() * ROTATION_SPEED);
        transform.rotate_y(0.7 * time.delta_secs() * ROTATION_SPEED);
    }
}

// Create resource to hold the ratatui terminal
#[derive(Resource, Deref, DerefMut)]
struct SoftTerminal(Terminal<SoftBackend>);
impl Default for SoftTerminal {
    fn default() -> Self {
        let mut backend = SoftBackend::new_with_font(15, 15, 16, FONT_DATA);
        //backend.set_font_size(12);
        Self(Terminal::new(backend).unwrap())
    }
}

pub fn draw_computer_screen(frame: &mut Frame) {
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
}

pub fn new_computer_screen(frame: &mut Frame) {
    let area = frame.area();
    let textik = format!("Hello bevy! The window area is {}", area);
    frame.render_widget(
        Paragraph::new(textik)
            .block(Block::new().title("Ratatui").borders(Borders::ALL))
            .white()
            .on_red()
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn computer_test(
    key: Res<ButtonInput<KeyCode>>,
    mut softatui: ResMut<SoftTerminal>,
    proc_material: Res<MyProcGenMaterial>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    trace!("SYSTEM: computer_test");

    if key.just_pressed(KeyCode::KeyK) {
        println!("LMAO");
        softatui.draw(new_computer_screen).expect("oops");

        let width = softatui.backend().get_pixmap_width() as u32;
        let height = softatui.backend().get_pixmap_height() as u32;
        let data = softatui.backend().get_pixmap_data_as_rgba();
        let material = materials
            .get_mut(&proc_material.0)
            .expect("material not found!");

        let image = images
            .get_mut(material.base_color_texture.as_ref().unwrap().id())
            .expect("Image not found!");

        let mut temp = Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        temp.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        *image = temp;
    }
}
