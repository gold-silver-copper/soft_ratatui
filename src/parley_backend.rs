//! Parley + Vello rasterization backend for [`SoftBackend`].

use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::sync::{Arc, mpsc};

use crate::SoftBackend;
use crate::colors::*;
use crate::pixmap::RgbPixmap;
use crate::soft_backend::RasterBackend;
use crate::soft_backend::{BlinkConfig, CursorConfig};
use parley::fontique::Blob;
use parley::layout::PositionedLayoutItem;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontStack, FontStyle, FontWeight, Layout,
    LayoutContext, LineHeight, StyleProperty,
};
use ratatui_core::backend::Backend;
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;
use ratatui_core::style::Modifier;
use rustc_hash::{FxHashMap, FxHashSet};
use unicode_width::UnicodeWidthStr;
use vello::kurbo::Affine;
use vello::peniko::{Brush, Color, Fill};
use vello::wgpu;
use vello::{AaConfig, AaSupport, Glyph, Renderer, RendererOptions, Scene};

#[derive(Clone, Copy, Debug, Default)]
struct TextMetrics {
    cell_width: f32,
    cell_height: f32,
    baseline: f32,
    underline_position: f32,
    underline_thickness: f32,
    strikeout_position: f32,
    strikeout_thickness: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayoutKey {
    text: Box<str>,
    bold: bool,
    italic: bool,
    font_size_bits: u32,
}

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    scratch_width: u32,
    scratch_height: u32,
    padded_bytes_per_row: u32,
}

/// Raster backend built on `parley` and `vello`.
pub struct ParleyText {
    font_context: FontContext,
    layout_context: LayoutContext<[u8; 4]>,
    family_stack: Arc<[FontFamily<'static>]>,
    font_size: f32,
    metrics: TextMetrics,
    locale: Option<String>,
    cache: FxHashMap<LayoutKey, Arc<Layout<[u8; 4]>>>,
    gpu: GpuState,
}

impl RasterBackend for ParleyText {
    fn draw_cell(
        &mut self,
        xik: u16,
        yik: u16,
        rat_cell: &Cell,
        always_redraw_list: &mut FxHashSet<(u16, u16)>,
        blinking_fast: bool,
        blinking_slow: bool,
        char_width: usize,
        char_height: usize,
        rgb_pixmap: &mut RgbPixmap,
    ) {
        let mut rat_fg = rat_cell.fg;
        let rat_bg = rat_cell.bg;
        if rat_cell.modifier.contains(Modifier::HIDDEN) {
            rat_fg = rat_bg;
        }

        let (mut fg_color, mut bg_color) = if rat_cell.modifier.contains(Modifier::REVERSED) {
            (rat_to_rgb(&rat_bg, false), rat_to_rgb(&rat_fg, true))
        } else {
            (rat_to_rgb(&rat_fg, true), rat_to_rgb(&rat_bg, false))
        };

        if rat_cell.modifier.contains(Modifier::DIM) {
            (fg_color, bg_color) = (dim_rgb(fg_color), dim_rgb(bg_color));
        }

        let begin_x = xik as usize * char_width;
        let begin_y = yik as usize * char_height;
        let display_width = UnicodeWidthStr::width(rat_cell.symbol()).clamp(1, 2);
        let pixel_width = char_width * display_width;

        rgb_pixmap.fill_rect(begin_x, begin_y, pixel_width, char_height, bg_color);

        if rat_cell.modifier.contains(Modifier::SLOW_BLINK) {
            always_redraw_list.insert((xik, yik));
            if blinking_slow {
                fg_color = bg_color;
            }
        }
        if rat_cell.modifier.contains(Modifier::RAPID_BLINK) {
            always_redraw_list.insert((xik, yik));
            if blinking_fast {
                fg_color = bg_color;
            }
        }

        let text_symbol = rat_cell.symbol().to_string();
        if text_symbol.trim().is_empty() {
            self.draw_decorations(
                rgb_pixmap,
                begin_x,
                begin_y,
                pixel_width,
                fg_color,
                rat_cell,
            );
            return;
        }

        let layout = self.shape_text(
            text_symbol.clone(),
            rat_cell.modifier.contains(Modifier::BOLD),
            rat_cell.modifier.contains(Modifier::ITALIC),
        );

        if let Ok(rgba) = self.render_layout_to_rgba(
            &layout,
            pixel_width as u32,
            char_height as u32,
            fg_color,
            bg_color,
        ) {
            for py in 0..char_height {
                let row_offset = py * pixel_width * 4;
                for px in 0..pixel_width {
                    let offset = row_offset + px * 4;
                    rgb_pixmap.put_pixel(
                        begin_x + px,
                        begin_y + py,
                        [rgba[offset], rgba[offset + 1], rgba[offset + 2]],
                    );
                }
            }
        }

        self.draw_decorations(
            rgb_pixmap,
            begin_x,
            begin_y,
            pixel_width,
            fg_color,
            rat_cell,
        );
    }
}

impl ParleyText {
    fn shape_text(&mut self, text: String, bold: bool, italic: bool) -> Arc<Layout<[u8; 4]>> {
        let key = LayoutKey {
            text: text.clone().into_boxed_str(),
            bold,
            italic,
            font_size_bits: self.font_size.to_bits(),
        };
        if let Some(layout) = self.cache.get(&key) {
            return Arc::clone(layout);
        }

        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, &text, 1.0, true);
        builder.push_default(FontStack::from(&self.family_stack[..]));
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::FontStyle(if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        builder.push_default(StyleProperty::FontWeight(if bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        }));
        builder.push_default(StyleProperty::Locale(self.locale.as_deref()));
        builder.push_default(LineHeight::Absolute(self.metrics.cell_height.max(1.0)));

        let mut layout = builder.build(&text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        let layout = Arc::new(layout);
        self.cache.insert(key, Arc::clone(&layout));
        layout
    }

    fn render_layout_to_rgba(
        &mut self,
        layout: &Layout<[u8; 4]>,
        width: u32,
        height: u32,
        fg_color: [u8; 3],
        bg_color: [u8; 3],
    ) -> Result<Vec<u8>, String> {
        self.gpu.ensure_target(width, height);

        let mut scene = Scene::new();
        let brush = Brush::Solid(Color::from_rgb8(fg_color[0], fg_color[1], fg_color[2]));
        let transform = Affine::translate(self.snapped_layout_translation(layout));
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let mut pen_x = glyph_run.offset();
                let pen_y = glyph_run.baseline();
                let normalized_coords = run.normalized_coords().iter().copied().collect::<Vec<_>>();
                scene
                    .draw_glyphs(run.font())
                    .font_size(run.font_size())
                    .hint(true)
                    .normalized_coords(&normalized_coords)
                    .brush(&brush)
                    .transform(transform)
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let positioned = Glyph {
                                id: glyph.id as u32,
                                x: pen_x + glyph.x,
                                y: pen_y - glyph.y,
                            };
                            pen_x += glyph.advance;
                            positioned
                        }),
                    );
            }
        }

        self.gpu.render_scene(
            &scene,
            width,
            height,
            Color::from_rgb8(bg_color[0], bg_color[1], bg_color[2]),
        )
    }

    fn snapped_layout_translation(&self, layout: &Layout<[u8; 4]>) -> (f64, f64) {
        let mut baseline = self.metrics.baseline;
        let mut offset = 0.0;

        if let Some((glyph_offset, glyph_baseline)) = layout.lines().find_map(|line| {
            line.items().find_map(|item| match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    Some((glyph_run.offset(), glyph_run.baseline()))
                }
                _ => None,
            })
        }) {
            offset = glyph_offset;
            baseline = glyph_baseline;
        }

        (
            f64::from(offset.round() - offset),
            f64::from(baseline.round() - baseline),
        )
    }

    fn draw_decorations(
        &self,
        rgb_pixmap: &mut RgbPixmap,
        begin_x: usize,
        begin_y: usize,
        pixel_width: usize,
        color: [u8; 3],
        rat_cell: &Cell,
    ) {
        if rat_cell.modifier.contains(Modifier::UNDERLINED) {
            let y = begin_y
                + (self.metrics.underline_position.round() as isize)
                    .saturating_neg()
                    .unsigned_abs();
            let y = y
                .min(begin_y + self.metrics.cell_height.round() as usize)
                .saturating_sub(1);
            let thickness = self.metrics.underline_thickness.round().max(1.0) as usize;
            rgb_pixmap.fill_rect(begin_x, y, pixel_width, thickness, color);
        }
        if rat_cell.modifier.contains(Modifier::CROSSED_OUT) {
            let y = begin_y
                + (self.metrics.strikeout_position.round() as isize)
                    .saturating_neg()
                    .unsigned_abs();
            let y = y
                .min(begin_y + self.metrics.cell_height.round() as usize)
                .saturating_sub(1);
            let thickness = self.metrics.strikeout_thickness.round().max(1.0) as usize;
            rgb_pixmap.fill_rect(begin_x, y, pixel_width, thickness, color);
        }
    }

    fn measure_metrics(
        font_context: &mut FontContext,
        layout_context: &mut LayoutContext<[u8; 4]>,
        family_stack: &[FontFamily<'static>],
        font_size: f32,
        locale: Option<&str>,
    ) -> TextMetrics {
        let sample = "M";
        let mut builder = layout_context.ranged_builder(font_context, sample, 1.0, true);
        builder.push_default(FontStack::from(family_stack));
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::Locale(locale));

        let mut layout = builder.build(sample);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        let line = layout.lines().next().expect("sample line");
        let run_metrics = line
            .items()
            .find_map(|item| match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => Some(*glyph_run.run().metrics()),
                _ => None,
            })
            .unwrap_or_default();

        TextMetrics {
            cell_width: layout.full_width().round().max(1.0),
            cell_height: line.metrics().line_height.round().max(1.0),
            baseline: line.metrics().baseline,
            underline_position: run_metrics.underline_offset,
            underline_thickness: run_metrics.underline_size.max(1.0),
            strikeout_position: run_metrics.strikethrough_offset,
            strikeout_thickness: run_metrics.strikethrough_size.max(1.0),
        }
    }
}

impl SoftBackend<ParleyText> {
    /// Sets a new font size for the terminal image.
    pub fn set_font_size(&mut self, font_size: i32) {
        self.raster_backend.font_size = font_size as f32;
        self.raster_backend.metrics = ParleyText::measure_metrics(
            &mut self.raster_backend.font_context,
            &mut self.raster_backend.layout_context,
            &self.raster_backend.family_stack,
            self.raster_backend.font_size,
            self.raster_backend.locale.as_deref(),
        );
        self.raster_backend.cache.clear();
        self.char_width = self.raster_backend.metrics.cell_width.round().max(1.0) as usize;
        self.char_height = self.raster_backend.metrics.cell_height.round().max(1.0) as usize;
        self.rgb_pixmap = RgbPixmap::new(
            self.char_width * self.buffer.area.width as usize,
            self.char_height * self.buffer.area.height as usize,
        );
        self.redraw();
    }

    /// Creates a new software backend with the given font data.
    pub fn new(width: u16, height: u16, font_size: i32, font_data: &[u8]) -> Self {
        let mut font_context = FontContext::new();
        let family_stack = register_font_and_build_family_stack(&mut font_context, font_data);
        let locale = text_locale();
        let mut layout_context = LayoutContext::default();
        let metrics = ParleyText::measure_metrics(
            &mut font_context,
            &mut layout_context,
            &family_stack,
            font_size as f32,
            locale.as_deref(),
        );
        let char_width = metrics.cell_width.round().max(1.0) as usize;
        let char_height = metrics.cell_height.round().max(1.0) as usize;
        let rgb_pixmap = RgbPixmap::new(char_width * width as usize, char_height * height as usize);

        let mut backend = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            cursor_pos: (0, 0),
            cursor_config: CursorConfig::default(),
            raster_backend: ParleyText {
                font_context,
                layout_context,
                family_stack,
                font_size: font_size as f32,
                metrics,
                locale,
                cache: FxHashMap::default(),
                gpu: GpuState::new(char_width as u32, char_height as u32)
                    .expect("failed to initialize vello renderer"),
            },
            rgb_pixmap,
            char_width,
            char_height,
            frame_count: 0,
            blink_config: BlinkConfig::default(),
            always_redraw_list: FxHashSet::default(),
            rendered_cursor: None,
        };
        _ = backend.clear();
        backend
    }
}

impl GpuState {
    fn new(width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .map_err(|err| format!("wgpu adapter request failed: {err}"))?;
        let maybe_features = wgpu::Features::CLEAR_TEXTURE | wgpu::Features::PIPELINE_CACHE;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: adapter.features() & maybe_features,
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
            experimental_features: Default::default(),
        }))
        .map_err(|err| format!("wgpu device request failed: {err}"))?;
        let renderer = Renderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                #[cfg(target_os = "macos")]
                num_init_threads: NonZeroUsize::new(1),
                #[cfg(not(target_os = "macos"))]
                num_init_threads: None,
                pipeline_cache: None,
            },
        )
        .map_err(|err| format!("vello renderer init failed: {err}"))?;
        let (texture, texture_view, readback, padded_bytes_per_row) =
            create_target_resources(&device, width, height);
        Ok(Self {
            device,
            queue,
            renderer,
            texture,
            texture_view,
            readback,
            scratch_width: width,
            scratch_height: height,
            padded_bytes_per_row,
        })
    }

    fn ensure_target(&mut self, width: u32, height: u32) {
        if self.scratch_width == width && self.scratch_height == height {
            return;
        }
        let (texture, texture_view, readback, padded_bytes_per_row) =
            create_target_resources(&self.device, width, height);
        self.texture = texture;
        self.texture_view = texture_view;
        self.readback = readback;
        self.scratch_width = width;
        self.scratch_height = height;
        self.padded_bytes_per_row = padded_bytes_per_row;
    }

    fn render_scene(
        &mut self,
        scene: &Scene,
        width: u32,
        height: u32,
        bg: Color,
    ) -> Result<Vec<u8>, String> {
        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                &self.texture_view,
                &vello::RenderParams {
                    base_color: bg,
                    width,
                    height,
                    antialiasing_method: AaConfig::Msaa8,
                },
            )
            .map_err(|err| format!("render_to_texture failed: {err}"))?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = self.readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|err| format!("device poll failed: {err}"))?;
        receiver
            .recv()
            .map_err(|err| format!("map_async channel failed: {err}"))?
            .map_err(|err| format!("map_async failed: {err}"))?;

        let mapped = slice.get_mapped_range();
        let row_len = width as usize * 4;
        let mut rgba = vec![0; row_len * height as usize];
        for y in 0..height as usize {
            let src = y * self.padded_bytes_per_row as usize;
            let dst = y * row_len;
            rgba[dst..dst + row_len].copy_from_slice(&mapped[src..src + row_len]);
        }
        drop(mapped);
        self.readback.unmap();
        Ok(rgba)
    }
}

fn create_target_resources(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Buffer, u32) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let padded_bytes_per_row = align_to(width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: padded_bytes_per_row as u64 * height as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    (texture, texture_view, readback, padded_bytes_per_row)
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn register_font_and_build_family_stack(
    font_context: &mut FontContext,
    font_data: &[u8],
) -> Arc<[FontFamily<'static>]> {
    let before = font_context
        .collection
        .family_names()
        .map(str::to_owned)
        .collect::<FxHashSet<_>>();
    font_context
        .collection
        .register_fonts(Blob::new(Arc::new(font_data.to_vec())), None);

    let mut families = font_context
        .collection
        .family_names()
        .filter(|family| !before.contains(*family))
        .map(|family| FontFamily::Named(Cow::Owned(family.to_owned())))
        .collect::<Vec<_>>();

    push_family(
        &mut families,
        FontFamily::Generic(parley::GenericFamily::Monospace),
    );
    push_family(
        &mut families,
        FontFamily::Generic(parley::GenericFamily::SystemUi),
    );
    push_family(
        &mut families,
        FontFamily::Generic(parley::GenericFamily::Emoji),
    );

    Arc::from(families)
}

fn push_family(families: &mut Vec<FontFamily<'static>>, family: FontFamily<'static>) {
    if !families.contains(&family) {
        families.push(family);
    }
}

fn text_locale() -> Option<String> {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok())
        .and_then(|locale| normalize_locale(&locale))
}

fn normalize_locale(locale: &str) -> Option<String> {
    let locale = locale.trim();
    if locale.is_empty() || matches!(locale, "C" | "POSIX") {
        return None;
    }
    let locale = locale
        .split_once('.')
        .map(|(locale, _)| locale)
        .unwrap_or(locale);
    let locale = locale
        .split_once('@')
        .map(|(locale, _)| locale)
        .unwrap_or(locale);
    let locale = locale.replace('_', "-");
    (!locale.is_empty()).then_some(locale)
}
