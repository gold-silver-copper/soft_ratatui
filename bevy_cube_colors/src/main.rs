//! Shows how to render UI to a texture. Useful for displaying UI in 3D space.
use bevy::{
    color::palettes::css::GOLD,
    prelude::*,
    render::{
        camera::RenderTarget,
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
};
use color_eyre::Result;
use embedded_graphics_unicodefonts::{
    mono_8x13_atlas, mono_8x13_bold_atlas, mono_8x13_italic_atlas,
};
use itertools::Itertools;
use palette::{Okhsv, Srgb, convert::FromColorUnclamped};
use ratatui::{Frame, Terminal};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::Color,
    text::Text,
    widgets::Widget,
};
use ratatui::{
    prelude::Stylize,
    style::{Modifier, Style},
};
use ratatui::{
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use soft_ratatui::{Bdf, EmbeddedGraphics, SoftBackend};
use std::{
    f32::consts::PI,
    iter::once,
    time::{Duration, Instant},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<SoftTerminal>()
        .init_resource::<Myappik>()
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

    let cube_size = 1.0;
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
        Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

const ROTATION_SPEED: f32 = 0.5;

fn rotator_system(time: Res<Time>, mut query: Query<&mut Transform, With<Cube>>) {
    for mut transform in &mut query {
        transform.rotate_x(1.0 * time.delta_secs() * ROTATION_SPEED);
        transform.rotate_y(0.7 * time.delta_secs() * ROTATION_SPEED);
    }
}
#[derive(Resource, Deref, DerefMut, Default)]
struct Myappik(MyApp);
// Create resource to hold the ratatui terminal
#[derive(Resource, Deref, DerefMut)]
struct SoftTerminal(Terminal<SoftBackend<EmbeddedGraphics>>);
impl Default for SoftTerminal {
    fn default() -> Self {
        let font_regular = mono_8x13_atlas();
        let font_italic = mono_8x13_italic_atlas();
        let font_bold = mono_8x13_bold_atlas();
        let backend = SoftBackend::<EmbeddedGraphics>::new(
            40,
            40,
            font_regular,
            Some(font_bold),
            Some(font_italic),
        );
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
    mut myapp: ResMut<Myappik>,
    proc_material: Res<MyProcGenMaterial>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if key.pressed(KeyCode::KeyK) {
        myapp.run(&mut softatui.0);
    }
    if key.just_pressed(KeyCode::KeyL) {
        softatui.resize(Rect::new(0, 0, 5, 5)).unwrap();
        println!("width {:#?}", softatui.backend().get_pixmap_width());
        softatui.clear();
    }
    if key.pressed(KeyCode::KeyL) {
        softatui.resize(Rect::new(0, 0, 5, 5)).expect("what");
        softatui.draw(draw).unwrap();
        softatui.clear();
    }

    //  softatui.draw(new_computer_screen).expect("oops");

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
    temp.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    *image = temp;
}

#[derive(Debug, Default)]
struct MyApp {
    /// The current state of the app (running or quit)
    state: AppState,

    /// A widget that displays the current frames per second
    fps_widget: FpsWidget,

    /// A widget that displays the full range of RGB colors that can be displayed in the terminal.
    colors_widget: ColorsWidget,
}
#[derive(Debug, Default, PartialEq, Eq)]
enum AppState {
    /// The app is running
    #[default]
    Running,

    /// The user has requested the app to quit
    Quit,
}

/// A widget that displays the current frames per second
#[derive(Debug)]
struct FpsWidget {
    /// The number of elapsed frames that have passed - used to calculate the fps
    frame_count: usize,

    /// The last instant that the fps was calculated
    last_instant: Instant,

    /// The current frames per second
    fps: Option<f32>,
}

/// A widget that displays the full range of RGB colors that can be displayed in the terminal.
///
/// This widget is animated and will change colors over time.
#[derive(Debug, Default)]
struct ColorsWidget {
    /// The colors to render - should be double the height of the area as we render two rows of
    /// pixels for each row of the widget using the half block character. This is computed any time
    /// the size of the widget changes.
    colors: Vec<Vec<Color>>,

    /// the number of elapsed frames that have passed - used to animate the colors by shifting the
    /// x index by the frame number
    frame_count: usize,
}
impl MyApp {
    /// Run the app
    ///
    /// This is the main event loop for the app.
    pub fn run(&mut self, terminal: &mut Terminal<SoftBackend<EmbeddedGraphics>>) {
        terminal.draw(|frame| frame.render_widget(self, frame.area()));
    }

    const fn is_running(&self) -> bool {
        matches!(self.state, AppState::Running)
    }
}

/// Implement the Widget trait for &mut App so that it can be rendered
///
/// This is implemented on a mutable reference so that the app can update its state while it is
/// being rendered. This allows the fps widget to update the fps calculation and the colors widget
/// to update the colors to render.
impl Widget for &mut MyApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use Constraint::{Length, Min};
        let [top, colors] = Layout::vertical([Length(1), Min(0)]).areas(area);
        let [title, fps] = Layout::horizontal([Min(0), Length(8)]).areas(top);
        Text::from("colors_rgb ")
            .centered()
            .black()
            .render(title, buf);
        self.fps_widget.render(fps, buf);
        self.colors_widget.render(colors, buf);
    }
}
/// Default impl for `FpsWidget`
///
/// Manual impl is required because we need to initialize the `last_instant` field to the current
/// instant.
impl Default for FpsWidget {
    fn default() -> Self {
        Self {
            frame_count: 0,
            last_instant: Instant::now(),
            fps: None,
        }
    }
}

/// Widget impl for `FpsWidget`
///
/// This is implemented on a mutable reference so that we can update the frame count and fps
/// calculation while rendering.
impl Widget for &mut FpsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.calculate_fps();
        if let Some(fps) = self.fps {
            let text = format!("{fps:.2} fps");
            Text::from(text).black().slow_blink().render(area, buf);
        }
    }
}

impl FpsWidget {
    /// Update the fps calculation.
    ///
    /// This updates the fps once a second, but only if the widget has rendered at least 2 frames
    /// since the last calculation. This avoids noise in the fps calculation when rendering on slow
    /// machines that can't render at least 2 frames per second.
    #[allow(clippy::cast_precision_loss)]
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

/// Widget impl for `ColorsWidget`
///
/// This is implemented on a mutable reference so that we can update the frame count and store a
/// cached version of the colors to render instead of recalculating them every frame.
impl Widget for &mut ColorsWidget {
    /// Render the widget
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.setup_colors(area);
        let colors = &self.colors;
        for (xi, x) in (area.left()..area.right()).enumerate() {
            // animate the colors by shifting the x index by the frame number
            let xi = (xi + self.frame_count) % (area.width as usize);
            for (yi, y) in (area.top()..area.bottom()).enumerate() {
                // render a half block character for each row of pixels with the foreground color
                // set to the color of the pixel and the background color set to the color of the
                // pixel below it
                let fg = colors[yi * 2][xi];
                let bg = colors[yi * 2 + 1][xi];
                buf[Position::new(x, y)].set_char('▀').set_fg(fg).set_bg(bg);
            }
        }
        self.frame_count += 1;
    }
}

impl ColorsWidget {
    /// Setup the colors to render.
    ///
    /// This is called once per frame to setup the colors to render. It caches the colors so that
    /// they don't need to be recalculated every frame.
    #[allow(clippy::cast_precision_loss)]
    fn setup_colors(&mut self, size: Rect) {
        let Rect { width, height, .. } = size;
        // double the height because each screen row has two rows of half block pixels
        let height = height as usize * 2;
        let width = width as usize;
        // only update the colors if the size has changed since the last time we rendered
        if self.colors.len() == height && self.colors[0].len() == width {
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
                let color = Color::Rgb(color.red, color.green, color.blue);
                row.push(color);
            }
            self.colors.push(row);
        }
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
