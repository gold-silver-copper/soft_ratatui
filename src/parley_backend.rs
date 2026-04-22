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
use vello::kurbo::{Affine, Rect as KRect};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FontVariant {
    Normal,
    Bold,
    Italic,
    BoldItalic,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayoutKey {
    text: Box<str>,
    variant: FontVariant,
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

#[derive(Clone, Copy, Debug)]
struct ResolvedCellStyle {
    fg_color: [u8; 3],
    bg_color: [u8; 3],
    display_width: usize,
}

/// Raster backend built on `parley` and `vello`.
pub struct ParleyText {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    family_stack: Arc<[FontFamily<'static>]>,
    font_size: f32,
    metrics: TextMetrics,
    locale: Option<String>,
    cache: FxHashMap<LayoutKey, Arc<Layout<()>>>,
    gpu: GpuState,
}

impl RasterBackend for ParleyText {
    fn prefers_full_frame(&self) -> bool {
        true
    }

    fn draw_frame(
        &mut self,
        buffer: &Buffer,
        always_redraw_list: &mut FxHashSet<(u16, u16)>,
        blinking_fast: bool,
        blinking_slow: bool,
        char_width: usize,
        char_height: usize,
        rgb_pixmap: &mut RgbPixmap,
    ) {
        always_redraw_list.clear();

        let width = (buffer.area.width as usize * char_width) as u32;
        let height = (buffer.area.height as usize * char_height) as u32;
        self.gpu.ensure_target(width, height);
        let mut scene = Scene::new();

        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                let style = self.resolve_cell_style(cell, blinking_fast, blinking_slow);
                if cell.modifier.contains(Modifier::SLOW_BLINK)
                    || cell.modifier.contains(Modifier::RAPID_BLINK)
                {
                    always_redraw_list.insert((x, y));
                }

                let begin_x = x as usize * char_width;
                let begin_y = y as usize * char_height;
                let pixel_width = char_width * style.display_width;

                rgb_pixmap.fill_rect(begin_x, begin_y, pixel_width, char_height, style.bg_color);

                if !cell.symbol().is_empty() {
                    let layout = self.shape_text(
                        cell.symbol().to_string(),
                        font_variant_from_style(
                            cell.modifier.contains(Modifier::BOLD),
                            cell.modifier.contains(Modifier::ITALIC),
                        ),
                    );
                    self.paint_layout(&mut scene, &layout, begin_x as f32, begin_y as f32, style.fg_color);
                }

                self.draw_decorations_scene(
                    &mut scene,
                    begin_x,
                    begin_y,
                    pixel_width,
                    style.fg_color,
                    cell,
                );
            }
        }

        if let Ok(rgba) =
            self.gpu
                .render_scene(&scene, width, height, Color::from_rgba8(0, 0, 0, 0))
        {
            rgb_pixmap.blend_from_rgba(&rgba);
        }
    }

    fn draw_cell(
        &mut self,
        x: u16,
        y: u16,
        rat_cell: &Cell,
        always_redraw_list: &mut FxHashSet<(u16, u16)>,
        blinking_fast: bool,
        blinking_slow: bool,
        char_width: usize,
        char_height: usize,
        rgb_pixmap: &mut RgbPixmap,
    ) {
        let style = self.resolve_cell_style(rat_cell, blinking_fast, blinking_slow);
        let begin_x = x as usize * char_width;
        let begin_y = y as usize * char_height;
        let pixel_width = char_width * style.display_width;

        rgb_pixmap.fill_rect(begin_x, begin_y, pixel_width, char_height, style.bg_color);

        if rat_cell.modifier.contains(Modifier::SLOW_BLINK) {
            always_redraw_list.insert((x, y));
        }
        if rat_cell.modifier.contains(Modifier::RAPID_BLINK) {
            always_redraw_list.insert((x, y));
        }

        if rat_cell.symbol().is_empty() {
            self.draw_decorations(rgb_pixmap, begin_x, begin_y, pixel_width, style.fg_color, rat_cell);
            return;
        }

        let layout = self.shape_text(
            rat_cell.symbol().to_string(),
            font_variant_from_style(
                rat_cell.modifier.contains(Modifier::BOLD),
                rat_cell.modifier.contains(Modifier::ITALIC),
            ),
        );

        if let Ok(rgba) =
            self.render_layout_to_rgba(&layout, pixel_width as u32, char_height as u32, style.fg_color)
        {
            self.blit_rgba(rgb_pixmap, begin_x, begin_y, pixel_width, char_height, &rgba);
        }

        self.draw_decorations(rgb_pixmap, begin_x, begin_y, pixel_width, style.fg_color, rat_cell);
    }
}

impl ParleyText {
    fn resolve_cell_style(
        &self,
        rat_cell: &Cell,
        blinking_fast: bool,
        blinking_slow: bool,
    ) -> ResolvedCellStyle {
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
        if rat_cell.modifier.contains(Modifier::SLOW_BLINK) && blinking_slow {
            fg_color = bg_color;
        }
        if rat_cell.modifier.contains(Modifier::RAPID_BLINK) && blinking_fast {
            fg_color = bg_color;
        }

        ResolvedCellStyle {
            fg_color,
            bg_color,
            display_width: UnicodeWidthStr::width(rat_cell.symbol()).clamp(1, 2),
        }
    }

    fn shape_text(&mut self, text: String, variant: FontVariant) -> Arc<Layout<()>> {
        let key = LayoutKey {
            text: text.clone().into_boxed_str(),
            variant,
            font_size_bits: self.font_size.to_bits(),
        };
        if let Some(layout) = self.cache.get(&key) {
            return Arc::clone(layout);
        }

        let (font_style, font_weight) = font_style(variant);
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, &text, 1.0, true);
        builder.push_default(FontStack::from(&self.family_stack[..]));
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::FontStyle(font_style));
        builder.push_default(StyleProperty::FontWeight(font_weight));
        builder.push_default(StyleProperty::Locale(self.locale.as_deref()));
        builder.push_default(LineHeight::Absolute(self.metrics.cell_height.max(1.0)));

        let mut layout = builder.build(&text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        let layout = Arc::new(layout);
        self.cache.insert(key, Arc::clone(&layout));
        layout
    }

    fn paint_layout(
        &self,
        scene: &mut Scene,
        layout: &Layout<()>,
        origin_x: f32,
        origin_y: f32,
        fg_color: [u8; 3],
    ) {
        let transform = Affine::translate((origin_x as f64, origin_y as f64));
        let brush = Brush::Solid(Color::from_rgb8(fg_color[0], fg_color[1], fg_color[2]));

        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();

                scene
                    .draw_glyphs(run.font())
                    .brush(&brush)
                    .hint(false)
                    .transform(transform)
                    .font_size(run.font_size())
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| scene_glyph_from_layout(&mut x, y, glyph)),
                    );
            }
        }
    }

    fn render_layout_to_rgba(
        &mut self,
        layout: &Layout<()>,
        width: u32,
        height: u32,
        fg_color: [u8; 3],
    ) -> Result<Vec<u8>, String> {
        self.gpu.ensure_target(width, height);
        let mut scene = Scene::new();
        self.paint_layout(&mut scene, layout, 0.0, 0.0, fg_color);
        self.gpu
            .render_scene(&scene, width, height, Color::from_rgba8(0, 0, 0, 0))
    }

    fn blit_rgba(
        &self,
        rgb_pixmap: &mut RgbPixmap,
        begin_x: usize,
        begin_y: usize,
        pixel_width: usize,
        char_height: usize,
        rgba: &[u8],
    ) {
        for py in 0..char_height {
            let src_row = &rgba[py * pixel_width * 4..(py + 1) * pixel_width * 4];
            let dst_offset = ((begin_y + py) * rgb_pixmap.width + begin_x) * 3;
            for (dst, src) in rgb_pixmap.data[dst_offset..dst_offset + pixel_width * 3]
                .chunks_exact_mut(3)
                .zip(src_row.chunks_exact(4))
            {
                dst.copy_from_slice(&src[..3]);
            }
        }
    }

    fn fill_scene_rect(
        &self,
        scene: &mut Scene,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: [u8; 3],
    ) {
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgb8(color[0], color[1], color[2]),
            None,
            &KRect::new(x, y, x + width, y + height),
        );
    }

    fn draw_decorations_scene(
        &self,
        scene: &mut Scene,
        begin_x: usize,
        begin_y: usize,
        pixel_width: usize,
        color: [u8; 3],
        rat_cell: &Cell,
    ) {
        if rat_cell.modifier.contains(Modifier::UNDERLINED) {
            let thickness = self.metrics.underline_thickness.max(1.0);
            let y =
                ((begin_y as f32 + self.metrics.baseline - self.metrics.underline_position) - thickness / 2.0)
                    .round()
                    .min(begin_y as f32 + self.metrics.cell_height - thickness);
            self.fill_scene_rect(
                scene,
                begin_x as f64,
                y as f64,
                pixel_width as f64,
                thickness as f64,
                color,
            );
        }
        if rat_cell.modifier.contains(Modifier::CROSSED_OUT) {
            let thickness = self.metrics.strikeout_thickness.max(1.0);
            let y = ((begin_y as f32 + self.metrics.baseline - self.metrics.strikeout_position)
                - thickness / 2.0)
                .round()
                .min(begin_y as f32 + self.metrics.cell_height - thickness);
            self.fill_scene_rect(
                scene,
                begin_x as f64,
                y as f64,
                pixel_width as f64,
                thickness as f64,
                color,
            );
        }
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
            let thickness = self.metrics.underline_thickness.round().max(1.0) as usize;
            let y = ((begin_y as f32 + self.metrics.baseline - self.metrics.underline_position)
                - thickness as f32 / 2.0)
                .round()
                .clamp(begin_y as f32, begin_y as f32 + self.metrics.cell_height - thickness as f32)
                as usize;
            rgb_pixmap.fill_rect(begin_x, y, pixel_width, thickness, color);
        }
        if rat_cell.modifier.contains(Modifier::CROSSED_OUT) {
            let thickness = self.metrics.strikeout_thickness.round().max(1.0) as usize;
            let y = ((begin_y as f32 + self.metrics.baseline - self.metrics.strikeout_position)
                - thickness as f32 / 2.0)
                .round()
                .clamp(begin_y as f32, begin_y as f32 + self.metrics.cell_height - thickness as f32)
                as usize;
            rgb_pixmap.fill_rect(begin_x, y, pixel_width, thickness, color);
        }
    }

    fn measure_metrics(
        font_context: &mut FontContext,
        layout_context: &mut LayoutContext<()>,
        family_stack: &[FontFamily<'static>],
        font_size: f32,
        locale: Option<&str>,
    ) -> TextMetrics {
        let mut metrics = TextMetrics::default();

        for variant in [
            FontVariant::Normal,
            FontVariant::Bold,
            FontVariant::Italic,
            FontVariant::BoldItalic,
        ] {
            let sample = "M";
            let (font_style, font_weight) = font_style(variant);
            let mut builder = layout_context.ranged_builder(font_context, sample, 1.0, true);
            builder.push_default(FontStack::from(family_stack));
            builder.push_default(StyleProperty::FontSize(font_size));
            builder.push_default(StyleProperty::FontStyle(font_style));
            builder.push_default(StyleProperty::FontWeight(font_weight));
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

            metrics.cell_width = metrics.cell_width.max(layout.full_width().floor().max(1.0));
            metrics.cell_height = metrics
                .cell_height
                .max(line.metrics().line_height.floor().max(1.0));
            metrics.baseline = metrics.baseline.max(line.metrics().baseline);
            metrics.underline_position = metrics.underline_position.max(run_metrics.underline_offset);
            metrics.underline_thickness =
                metrics.underline_thickness.max(run_metrics.underline_size.max(1.0));
            metrics.strikeout_position =
                metrics.strikeout_position.max(run_metrics.strikethrough_offset);
            metrics.strikeout_thickness = metrics
                .strikeout_thickness
                .max(run_metrics.strikethrough_size.max(1.0));
        }

        metrics
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
        Self::new_with_fonts(width, height, font_size, font_data, None, None, None)
    }

    /// Creates a new software backend with separate font data for each text variant.
    pub fn new_with_fonts(
        width: u16,
        height: u16,
        font_size: i32,
        font_regular: &[u8],
        font_bold: Option<&[u8]>,
        font_italic: Option<&[u8]>,
        font_bold_italic: Option<&[u8]>,
    ) -> Self {
        let mut font_context = FontContext::new();
        let family_stack = register_fonts_and_build_family_stack(
            &mut font_context,
            font_regular,
            font_bold,
            font_italic,
            font_bold_italic,
        );
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

fn font_variant_from_style(bold: bool, italic: bool) -> FontVariant {
    match (bold, italic) {
        (true, true) => FontVariant::BoldItalic,
        (true, false) => FontVariant::Bold,
        (false, true) => FontVariant::Italic,
        (false, false) => FontVariant::Normal,
    }
}

fn font_style(variant: FontVariant) -> (FontStyle, FontWeight) {
    match variant {
        FontVariant::Normal => (FontStyle::Normal, FontWeight::NORMAL),
        FontVariant::Bold => (FontStyle::Normal, FontWeight::BOLD),
        FontVariant::Italic => (FontStyle::Italic, FontWeight::NORMAL),
        FontVariant::BoldItalic => (FontStyle::Italic, FontWeight::BOLD),
    }
}

fn register_fonts_and_build_family_stack(
    font_context: &mut FontContext,
    font_regular: &[u8],
    font_bold: Option<&[u8]>,
    font_italic: Option<&[u8]>,
    font_bold_italic: Option<&[u8]>,
) -> Arc<[FontFamily<'static>]> {
    let before = font_context
        .collection
        .family_names()
        .map(str::to_owned)
        .collect::<FxHashSet<_>>();

    register_font(font_context, font_regular);
    if let Some(data) = font_bold {
        register_font(font_context, data);
    }
    if let Some(data) = font_italic {
        register_font(font_context, data);
    }
    if let Some(data) = font_bold_italic {
        register_font(font_context, data);
    }

    let mut families = font_context
        .collection
        .family_names()
        .filter(|family| !before.contains(*family))
        .map(|family| FontFamily::Named(Cow::Owned(family.to_owned())))
        .collect::<Vec<_>>();

    if families.is_empty() {
        families = font_context
            .collection
            .family_names()
            .map(|family| FontFamily::Named(Cow::Owned(family.to_owned())))
            .collect();
    }

    push_family(&mut families, FontFamily::Generic(parley::GenericFamily::Monospace));
    push_family(&mut families, FontFamily::Generic(parley::GenericFamily::SystemUi));
    push_family(&mut families, FontFamily::Generic(parley::GenericFamily::Emoji));

    Arc::from(families)
}

fn register_font(font_context: &mut FontContext, font_data: &[u8]) {
    font_context
        .collection
        .register_fonts(Blob::new(Arc::new(font_data.to_vec())), None);
}

fn push_family(families: &mut Vec<FontFamily<'static>>, family: FontFamily<'static>) {
    if !families.contains(&family) {
        families.push(family);
    }
}

fn scene_glyph_from_layout(cursor_x: &mut f32, baseline: f32, glyph: parley::layout::Glyph) -> Glyph {
    let positioned = Glyph {
        id: glyph.id as u32,
        x: *cursor_x + glyph.x,
        y: baseline - glyph.y,
    };
    *cursor_x += glyph.advance;
    positioned
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
