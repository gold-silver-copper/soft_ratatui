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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayoutKey {
    text: Box<str>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    font_size_bits: u32,
    fg_color: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FontVariant {
    Normal,
    Bold,
    Italic,
    BoldItalic,
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
    layout_context: LayoutContext<[u8; 4]>,
    family_stacks: [Arc<[FontFamily<'static>]>; 4],
    font_size: f32,
    metrics: TextMetrics,
    locale: Option<String>,
    cache: FxHashMap<LayoutKey, Arc<Layout<[u8; 4]>>>,
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

                self.fill_scene_rect(
                    &mut scene,
                    begin_x as f64,
                    begin_y as f64,
                    pixel_width as f64,
                    char_height as f64,
                    style.bg_color,
                );

                if !cell.symbol().is_empty() {
                    let layout = self.shape_text(
                        cell.symbol().to_string(),
                        cell.modifier.contains(Modifier::BOLD),
                        cell.modifier.contains(Modifier::ITALIC),
                        cell.modifier.contains(Modifier::UNDERLINED),
                        cell.modifier.contains(Modifier::CROSSED_OUT),
                        style.fg_color,
                    );
                    self.paint_layout(
                        &mut scene,
                        &layout,
                        begin_x as f32,
                        begin_y as f32,
                    );
                }
            }
        }

        if let Ok(rgba) = self
            .gpu
            .render_scene(&scene, width, height, Color::from_rgb8(0, 0, 0))
        {
            rgb_pixmap.copy_from_rgba(&rgba);
        }
    }

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
        let style = self.resolve_cell_style(rat_cell, blinking_fast, blinking_slow);
        let begin_x = xik as usize * char_width;
        let begin_y = yik as usize * char_height;
        let pixel_width = char_width * style.display_width;

        rgb_pixmap.fill_rect(begin_x, begin_y, pixel_width, char_height, style.bg_color);

        if rat_cell.modifier.contains(Modifier::SLOW_BLINK) {
            always_redraw_list.insert((xik, yik));
        }
        if rat_cell.modifier.contains(Modifier::RAPID_BLINK) {
            always_redraw_list.insert((xik, yik));
        }

        let text_symbol = rat_cell.symbol().to_string();
        if text_symbol.is_empty() {
            self.draw_decorations(
                rgb_pixmap,
                begin_x,
                begin_y,
                pixel_width,
                style.fg_color,
                rat_cell,
            );
            return;
        }

        let layout = self.shape_text(
            text_symbol.clone(),
            rat_cell.modifier.contains(Modifier::BOLD),
            rat_cell.modifier.contains(Modifier::ITALIC),
            rat_cell.modifier.contains(Modifier::UNDERLINED),
            rat_cell.modifier.contains(Modifier::CROSSED_OUT),
            style.fg_color,
        );

        if let Ok(rgba) = self.render_layout_to_rgba(
            &layout,
            pixel_width as u32,
            char_height as u32,
            style.bg_color,
        ) {
            self.blit_rgba(
                rgb_pixmap,
                begin_x,
                begin_y,
                pixel_width,
                char_height,
                &rgba,
            );
        }
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

    fn shape_text(
        &mut self,
        text: String,
        bold: bool,
        italic: bool,
        underline: bool,
        strikethrough: bool,
        fg_color: [u8; 3],
    ) -> Arc<Layout<[u8; 4]>> {
        let key = LayoutKey {
            text: text.clone().into_boxed_str(),
            bold,
            italic,
            underline,
            strikethrough,
            font_size_bits: self.font_size.to_bits(),
            fg_color,
        };
        if let Some(layout) = self.cache.get(&key) {
            return Arc::clone(layout);
        }

        let variant = font_variant_from_style(bold, italic);
        let family_stack = Arc::clone(&self.family_stacks[variant.as_index()]);

        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, &text, 1.0, true);
        builder.push_default(FontStack::from(&family_stack[..]));
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
        builder.push_default(StyleProperty::Brush([fg_color[0], fg_color[1], fg_color[2], 255]));
        builder.push_default(StyleProperty::Underline(underline));
        builder.push_default(StyleProperty::UnderlineBrush(
            underline.then_some([fg_color[0], fg_color[1], fg_color[2], 255]),
        ));
        builder.push_default(StyleProperty::Strikethrough(strikethrough));
        builder.push_default(StyleProperty::StrikethroughBrush(
            strikethrough.then_some([fg_color[0], fg_color[1], fg_color[2], 255]),
        ));

        let mut layout = builder.build(&text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        let layout = Arc::new(layout);
        self.cache.insert(key, Arc::clone(&layout));
        layout
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

    fn paint_layout(
        &self,
        scene: &mut Scene,
        layout: &Layout<[u8; 4]>,
        origin_x: f32,
        origin_y: f32,
    ) {
        let snapped = self.snapped_layout_translation(layout);
        let transform = Affine::translate((
            f64::from(origin_x) + snapped.0,
            f64::from(origin_y) + snapped.1,
        ));

        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let style = glyph_run.style();
                let brush = Brush::Solid(Color::from_rgba8(
                    style.brush[0],
                    style.brush[1],
                    style.brush[2],
                    style.brush[3],
                ));
                let mut pen_x = glyph_run.offset();
                let pen_y = glyph_run.baseline();
                scene
                    .draw_glyphs(run.font())
                    .font_size(run.font_size())
                    .hint(true)
                    .normalized_coords(run.normalized_coords())
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

                if let Some(decoration) = &style.underline {
                    let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
                    let size = decoration.size.unwrap_or(run.metrics().underline_size);
                    self.paint_decoration(scene, glyph_run.offset(), glyph_run.advance(), glyph_run.baseline(), decoration.brush, offset, size, origin_x, origin_y);
                }
                if let Some(decoration) = &style.strikethrough {
                    let offset = decoration
                        .offset
                        .unwrap_or(run.metrics().strikethrough_offset);
                    let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);
                    self.paint_decoration(scene, glyph_run.offset(), glyph_run.advance(), glyph_run.baseline(), decoration.brush, offset, size, origin_x, origin_y);
                }
            }
        }
    }

    fn render_layout_to_rgba(
        &mut self,
        layout: &Layout<[u8; 4]>,
        width: u32,
        height: u32,
        bg_color: [u8; 3],
    ) -> Result<Vec<u8>, String> {
        self.gpu.ensure_target(width, height);

        let mut scene = Scene::new();
        self.paint_layout(&mut scene, layout, 0.0, 0.0);

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

    fn paint_decoration(
        &self,
        scene: &mut Scene,
        glyph_offset: f32,
        glyph_advance: f32,
        glyph_baseline: f32,
        brush: [u8; 4],
        offset: f32,
        size: f32,
        origin_x: f32,
        origin_y: f32,
    ) {
        let y = origin_y + glyph_baseline - offset;
        let x = origin_x + glyph_offset;
        self.fill_scene_rect(
            scene,
            x as f64,
            y as f64,
            glyph_advance as f64,
            size.max(1.0) as f64,
            [brush[0], brush[1], brush[2]],
        );
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
            &self.raster_backend.family_stacks[FontVariant::Normal.as_index()],
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
        let family_stacks = build_family_stacks(
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
            &family_stacks[FontVariant::Normal.as_index()],
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
                family_stacks,
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

impl FontVariant {
    const fn as_index(self) -> usize {
        match self {
            Self::Normal => 0,
            Self::Bold => 1,
            Self::Italic => 2,
            Self::BoldItalic => 3,
        }
    }
}

fn font_variant_from_style(bold: bool, italic: bool) -> FontVariant {
    match (bold, italic) {
        (true, true) => FontVariant::BoldItalic,
        (true, false) => FontVariant::Bold,
        (false, true) => FontVariant::Italic,
        (false, false) => FontVariant::Normal,
    }
}

fn build_family_stacks(
    font_context: &mut FontContext,
    font_regular: &[u8],
    font_bold: Option<&[u8]>,
    font_italic: Option<&[u8]>,
    font_bold_italic: Option<&[u8]>,
) -> [Arc<[FontFamily<'static>]>; 4] {
    let normal = register_font_and_build_family_stack(font_context, font_regular);
    let bold = font_bold
        .map(|data| register_font_and_build_family_stack(font_context, data))
        .unwrap_or_else(|| Arc::clone(&normal));
    let italic = font_italic
        .map(|data| register_font_and_build_family_stack(font_context, data))
        .unwrap_or_else(|| Arc::clone(&normal));
    let bold_italic = font_bold_italic
        .map(|data| register_font_and_build_family_stack(font_context, data))
        .unwrap_or_else(|| {
            if font_bold.is_some() && font_italic.is_some() {
                Arc::clone(&bold)
            } else if font_bold.is_some() {
                Arc::clone(&bold)
            } else if font_italic.is_some() {
                Arc::clone(&italic)
            } else {
                Arc::clone(&normal)
            }
        });

    [normal, bold, italic, bold_italic]
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
