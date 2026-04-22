use std::time::{Duration, Instant};

use eframe::egui::{self, TextureHandle};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use soft_ratatui::rusttype::Font;
use soft_ratatui::{EmbeddedTTF, SoftBackend};

const MIN_FONT_SIZE: u32 = 8;
const MAX_FONT_SIZE: u32 = 25;
const INITIAL_FONT_SIZE: u32 = MIN_FONT_SIZE;

static FONT_DATA: &[u8] = include_bytes!("../../../assets/JetBrainsMono-Regular.ttf");
static FONT_BOLD_DATA: &[u8] = include_bytes!("../../../assets/JetBrainsMono-Bold.ttf");
static FONT_ITALIC_DATA: &[u8] = include_bytes!("../../../assets/JetBrainsMono-Italic.ttf");

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 1100.0]),
        ..Default::default()
    };

    eframe::run_native(
        "soft_ratatui font size cycle",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new()))),
    )
}

struct MyApp {
    terminal: Terminal<SoftBackend<EmbeddedTTF>>,
    texture: Option<TextureHandle>,
    started_at: Instant,
    frame_count: u64,
    current_font_size: u32,
}

impl MyApp {
    fn new() -> Self {
        let font_regular = Font::try_from_bytes(FONT_DATA).unwrap();
        let font_bold = Font::try_from_bytes(FONT_BOLD_DATA).unwrap();
        let font_italic = Font::try_from_bytes(FONT_ITALIC_DATA).unwrap();
        let backend = SoftBackend::<EmbeddedTTF>::new(
            100,
            50,
            INITIAL_FONT_SIZE,
            font_regular,
            Some(font_bold),
            Some(font_italic),
        );

        Self {
            terminal: Terminal::new(backend).unwrap(),
            texture: None,
            started_at: Instant::now(),
            frame_count: 0,
            current_font_size: INITIAL_FONT_SIZE,
        }
    }

    fn target_font_size(&self) -> u32 {
        let span = (MAX_FONT_SIZE - MIN_FONT_SIZE) as u64;
        let step = self.started_at.elapsed().as_secs() % (span * 2 + 1);

        if step <= span {
            MIN_FONT_SIZE + step as u32
        } else {
            MAX_FONT_SIZE - (step as u32 - span as u32)
        }
    }

    fn sync_font_size(&mut self) {
        let next_font_size = self.target_font_size();
        if next_font_size != self.current_font_size {
            self.terminal.backend_mut().set_font_size(next_font_size);
            self.current_font_size = next_font_size;
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_font_size();

        let font_size = self.current_font_size;
        let elapsed_seconds = self.started_at.elapsed().as_secs_f32();
        let frame_count = self.frame_count;

        self.terminal
            .draw(|frame| draw(frame, font_size, elapsed_seconds, frame_count))
            .unwrap();
        self.frame_count += 1;

        let image = egui::ColorImage::from_rgb(
            [
                self.terminal.backend().get_pixmap_width(),
                self.terminal.backend().get_pixmap_height(),
            ],
            self.terminal.backend().get_pixmap_data(),
        );

        match self.texture.as_mut() {
            Some(texture) => texture.set(image, Default::default()),
            None => {
                self.texture = Some(ctx.load_texture("font-size-cycle", image, Default::default()));
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                if let Some(texture) = &self.texture {
                    ui.image((texture.id(), texture.size_vec2()));
                }
            });
        });

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn draw(frame: &mut Frame, font_size: u32, elapsed_seconds: f32, frame_count: u64) {
    let [header_area, scene_area, sample_area] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(10),
        Constraint::Length(8),
    ])
    .areas(frame.area());

    let header = Paragraph::new(Text::from(vec![
        Line::from(vec![
            Span::styled(
                "embedded_ttf set_font_size() demo",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("font size: {font_size}"),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(
                "cycling once per second from 8 to 25 and back",
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("elapsed: "),
            Span::styled(
                format!("{elapsed_seconds:>5.1}s"),
                Style::default().fg(Color::White),
            ),
            Span::raw("  frame: "),
            Span::styled(
                format!("{frame_count:>6}"),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled(
                "watch the canvas, borders, and styled text stay coherent during resizes",
                Style::default().fg(Color::Gray),
            ),
        ]),
    ]))
    .block(Block::default().title("Status").borders(Borders::ALL))
    .wrap(Wrap { trim: false });
    frame.render_widget(header, header_area);

    let scene_block = Block::default()
        .title("Animated Scene")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let scene_inner = scene_block.inner(scene_area);
    frame.render_widget(scene_block, scene_area);
    frame.render_widget(
        Paragraph::new(animated_scene(
            scene_inner.width as usize,
            scene_inner.height as usize,
            frame_count,
        ))
        .wrap(Wrap { trim: false }),
        scene_inner,
    );

    let samples = Paragraph::new(Text::from(vec![
        Line::from("The quick brown fox jumps over the lazy dog 0123456789"),
        Line::from(vec![
            Span::raw("styles: "),
            Span::styled("regular", Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(
                "bold",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "italic",
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::raw("  "),
            Span::styled(
                "bold italic",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ),
        ]),
        Line::from(vec![
            Span::styled("resize signal ", Style::default().fg(Color::Yellow)),
            Span::styled(progress_bar(font_size), Style::default().fg(Color::Cyan)),
        ]),
        Line::from("Every second the backend recalculates cell size and redraws the pixmap."),
    ]))
    .block(Block::default().title("Sample Text").borders(Borders::ALL))
    .wrap(Wrap { trim: false });
    frame.render_widget(samples, sample_area);
}

fn animated_scene(width: usize, height: usize, frame_count: u64) -> Text<'static> {
    if width == 0 || height == 0 {
        return Text::default();
    }

    let mut lines = Vec::with_capacity(height);
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2);

    let ball_x = if inner_width == 0 {
        0
    } else {
        1 + bounce(frame_count / 2, inner_width)
    };
    let ball_y = if inner_height == 0 {
        0
    } else {
        1 + bounce(frame_count / 3, inner_height)
    };

    for row in 0..height {
        let mut chars = vec![' '; width];

        if row == 0 || row + 1 == height {
            chars.fill('=');
        } else {
            chars[0] = '|';
            chars[width - 1] = '|';
        }

        if inner_width > 0 && row > 0 && row + 1 < height {
            let sweep = 1 + ((frame_count as usize + row * 3) % inner_width);
            chars[sweep] = if row % 2 == 0 { '*' } else { '+' };
        }

        if row == ball_y && ball_x < width {
            chars[ball_x] = '@';
            if ball_x > 1 {
                chars[ball_x - 1] = 'o';
            }
            if ball_x > 2 {
                chars[ball_x - 2] = '.';
            }
        }

        lines.push(Line::from(chars.into_iter().collect::<String>()));
    }

    Text::from(lines)
}

fn bounce(step: u64, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }

    let travel = len - 1;
    let period = travel * 2;
    let offset = step as usize % period;

    if offset <= travel {
        offset
    } else {
        period - offset
    }
}

fn progress_bar(font_size: u32) -> String {
    let filled = (font_size - MIN_FONT_SIZE + 1) as usize;
    let total = (MAX_FONT_SIZE - MIN_FONT_SIZE + 1) as usize;
    let mut bar = String::with_capacity(total + 2);
    bar.push('[');
    for index in 0..total {
        bar.push(if index < filled { '#' } else { '-' });
    }
    bar.push(']');
    bar
}
