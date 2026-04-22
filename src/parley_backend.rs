//! Parley + Vello rasterization backend for [`SoftBackend`].

use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::sync::{Arc, mpsc};

use crate::SoftBackend;
use crate::colors::*;
use crate::pixmap::RgbPixmap;
use crate::soft_backend::RasterBackend;
use crate::soft_backend::{BlinkConfig, CursorConfig};
use parley::fontique::{Blob, FallbackKey, FamilyId};
use parley::layout::PositionedLayoutItem;
use parley::swash::text::Codepoint as _;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontStack, FontStyle, FontWeight,
    GenericFamily, Layout, LayoutContext, LineHeight, StyleProperty,
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
    descent: f32,
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
enum LayoutTextKey {
    Char(char),
    String(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayoutKey {
    text: LayoutTextKey,
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
    rgba_scratch: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct ResolvedCellStyle {
    fg_color: [u8; 3],
    bg_color: [u8; 3],
    display_width: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecorationKind {
    Underline,
    Strikeout,
}

#[derive(Clone, Copy, Debug)]
struct DecorationRun {
    x: f64,
    end_x: f64,
    y: f64,
    height: f64,
    color: [u8; 3],
}

/// Raster backend built on `parley` and `vello`.
pub struct ParleyText {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    family_stack: Arc<[FontFamily<'static>]>,
    font_size: f32,
    metrics: TextMetrics,
    locale: Option<String>,
    fallback_search_families: Arc<[FamilyId]>,
    checked_fallbacks: FxHashSet<(FallbackKey, char)>,
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

                self.fill_scene_rect(
                    &mut scene,
                    begin_x as f64,
                    begin_y as f64,
                    pixel_width as f64,
                    char_height as f64,
                    style.bg_color,
                );
            }
        }

        for y in 0..buffer.area.height {
            let mut underline_run = None;
            let mut strikeout_run = None;
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                let style = self.resolve_cell_style(cell, blinking_fast, blinking_slow);
                let begin_x = x as usize * char_width;
                let begin_y = y as usize * char_height;
                let pixel_width = char_width * style.display_width;

                if !cell.symbol().is_empty() {
                    let layout = self.shape_cell_text(
                        cell.symbol(),
                        font_variant_from_style(
                            cell.modifier.contains(Modifier::BOLD),
                            cell.modifier.contains(Modifier::ITALIC),
                        ),
                    );
                    self.paint_layout(
                        &mut scene,
                        &layout,
                        begin_x as f32,
                        begin_y as f32,
                        style.fg_color,
                    );
                }

                self.update_decoration_run(
                    &mut scene,
                    &mut underline_run,
                    cell.modifier.contains(Modifier::UNDERLINED),
                    begin_x,
                    begin_y,
                    pixel_width,
                    style.fg_color,
                    DecorationKind::Underline,
                );
                self.update_decoration_run(
                    &mut scene,
                    &mut strikeout_run,
                    cell.modifier.contains(Modifier::CROSSED_OUT),
                    begin_x,
                    begin_y,
                    pixel_width,
                    style.fg_color,
                    DecorationKind::Strikeout,
                );
            }
            self.flush_decoration_run(&mut scene, &mut underline_run);
            self.flush_decoration_run(&mut scene, &mut strikeout_run);
        }

        if let Ok(rgba) = self
            .gpu
            .render_scene(&scene, width, height, Color::from_rgb8(0, 0, 0))
        {
            rgb_pixmap.copy_from_rgba(rgba);
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

        if rat_cell.modifier.contains(Modifier::SLOW_BLINK) {
            always_redraw_list.insert((x, y));
        }
        if rat_cell.modifier.contains(Modifier::RAPID_BLINK) {
            always_redraw_list.insert((x, y));
        }

        if let Ok(rgba) =
            self.render_cell_to_rgba(rat_cell, pixel_width as u32, char_height as u32, style)
        {
            blit_rgba(rgb_pixmap, begin_x, begin_y, pixel_width, char_height, rgba);
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

    fn shape_cell_text(&mut self, text: &str, variant: FontVariant) -> Arc<Layout<()>> {
        if let Some(character) = single_char(text) {
            self.shape_char(character, variant)
        } else {
            self.shape_text(text.to_owned(), variant)
        }
    }

    fn shape_char(&mut self, character: char, variant: FontVariant) -> Arc<Layout<()>> {
        let mut buffer = [0; 4];
        let text = character.encode_utf8(&mut buffer);
        self.ensure_fontique_fallbacks(text);

        let key = LayoutKey {
            text: LayoutTextKey::Char(character),
            variant,
            font_size_bits: self.font_size.to_bits(),
        };
        if let Some(layout) = self.cache.get(&key) {
            return Arc::clone(layout);
        }

        self.build_and_cache_layout(key, text, variant)
    }

    fn shape_text(&mut self, text: String, variant: FontVariant) -> Arc<Layout<()>> {
        self.ensure_fontique_fallbacks(&text);

        let key = LayoutKey {
            text: LayoutTextKey::String(text.clone().into_boxed_str()),
            variant,
            font_size_bits: self.font_size.to_bits(),
        };
        if let Some(layout) = self.cache.get(&key) {
            return Arc::clone(layout);
        }

        self.build_and_cache_layout(key, &text, variant)
    }

    fn build_and_cache_layout(
        &mut self,
        key: LayoutKey,
        text: &str,
        variant: FontVariant,
    ) -> Arc<Layout<()>> {
        let (font_style, font_weight) = font_style(variant);
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(FontStack::from(&self.family_stack[..]));
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::FontStyle(font_style));
        builder.push_default(StyleProperty::FontWeight(font_weight));
        builder.push_default(StyleProperty::Locale(self.locale.as_deref()));
        builder.push_default(LineHeight::Absolute(self.metrics.cell_height.max(1.0)));

        let mut layout = builder.build(text);
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
                        glyph_run
                            .glyphs()
                            .map(|glyph| scene_glyph_from_layout(&mut x, y, glyph)),
                    );
            }
        }
    }

    fn render_cell_to_rgba(
        &mut self,
        rat_cell: &Cell,
        width: u32,
        height: u32,
        style: ResolvedCellStyle,
    ) -> Result<&[u8], String> {
        self.gpu.ensure_target(width, height);
        let mut scene = Scene::new();
        self.fill_scene_rect(
            &mut scene,
            0.0,
            0.0,
            width as f64,
            height as f64,
            style.bg_color,
        );

        if !rat_cell.symbol().is_empty() {
            let layout = self.shape_cell_text(
                rat_cell.symbol(),
                font_variant_from_style(
                    rat_cell.modifier.contains(Modifier::BOLD),
                    rat_cell.modifier.contains(Modifier::ITALIC),
                ),
            );
            self.paint_layout(&mut scene, &layout, 0.0, 0.0, style.fg_color);
        }

        self.paint_cell_decorations(&mut scene, 0, 0, width as usize, style.fg_color, rat_cell);
        self.gpu
            .render_scene(&scene, width, height, Color::from_rgb8(0, 0, 0))
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

    fn paint_cell_decorations(
        &self,
        scene: &mut Scene,
        begin_x: usize,
        begin_y: usize,
        pixel_width: usize,
        color: [u8; 3],
        rat_cell: &Cell,
    ) {
        if rat_cell.modifier.contains(Modifier::UNDERLINED) {
            let (y, thickness) = self.decoration_geometry(begin_y, DecorationKind::Underline);
            self.fill_scene_rect(
                scene,
                begin_x as f64,
                y,
                pixel_width as f64,
                thickness,
                color,
            );
        }
        if rat_cell.modifier.contains(Modifier::CROSSED_OUT) {
            let (y, thickness) = self.decoration_geometry(begin_y, DecorationKind::Strikeout);
            self.fill_scene_rect(
                scene,
                begin_x as f64,
                y,
                pixel_width as f64,
                thickness,
                color,
            );
        }
    }

    fn update_decoration_run(
        &self,
        scene: &mut Scene,
        active_run: &mut Option<DecorationRun>,
        enabled: bool,
        begin_x: usize,
        begin_y: usize,
        pixel_width: usize,
        color: [u8; 3],
        kind: DecorationKind,
    ) {
        if !enabled {
            self.flush_decoration_run(scene, active_run);
            return;
        }

        let (y, height) = self.decoration_geometry(begin_y, kind);
        let x = begin_x as f64;
        let end_x = x + pixel_width as f64;

        match active_run {
            Some(run)
                if run.color == color && run.y == y && run.height == height && run.end_x == x =>
            {
                run.end_x = end_x;
            }
            _ => {
                self.flush_decoration_run(scene, active_run);
                *active_run = Some(DecorationRun {
                    x,
                    end_x,
                    y,
                    height,
                    color,
                });
            }
        }
    }

    fn flush_decoration_run(&self, scene: &mut Scene, active_run: &mut Option<DecorationRun>) {
        if let Some(run) = active_run.take() {
            self.fill_scene_rect(
                scene,
                run.x,
                run.y,
                run.end_x - run.x,
                run.height,
                run.color,
            );
        }
    }

    fn decoration_geometry(&self, begin_y: usize, kind: DecorationKind) -> (f64, f64) {
        let (position, thickness) = match kind {
            DecorationKind::Underline => (
                self.metrics.underline_position,
                self.metrics.underline_thickness.max(1.0),
            ),
            DecorationKind::Strikeout => (
                self.metrics.strikeout_position,
                self.metrics.strikeout_thickness.max(1.0),
            ),
        };
        let y = ((begin_y as f32 + self.metrics.baseline - position) - thickness / 2.0)
            .round()
            .min(begin_y as f32 + self.metrics.cell_height - thickness);
        (y as f64, thickness as f64)
    }

    fn ensure_fontique_fallbacks(&mut self, text: &str) {
        let mut changed = false;

        for character in text.chars() {
            let Some(key) = self.fallback_key_for_char(character) else {
                continue;
            };

            if !self.checked_fallbacks.insert((key, character)) {
                continue;
            }

            if self.fallbacks_support_character(key, character) {
                continue;
            }

            changed |= self.seed_fontique_fallbacks(key, character);
        }

        if changed {
            self.checked_fallbacks.clear();
            self.cache.clear();
        }
    }

    fn fallback_key_for_char(&self, character: char) -> Option<FallbackKey> {
        let script = fontique_script_for_char(character)?;
        let localized = self
            .locale
            .as_deref()
            .map(|locale| FallbackKey::from((script, locale)));
        match localized {
            Some(key) if key.is_tracked() => Some(key),
            _ => Some(FallbackKey::from(script)),
        }
    }

    fn fallbacks_support_character(&mut self, key: FallbackKey, character: char) -> bool {
        let fallback_families = self
            .font_context
            .collection
            .fallback_families(key)
            .collect::<Vec<_>>();
        let mut buffer = [0; 4];
        let character_text = character.encode_utf8(&mut buffer);
        fallback_families
            .into_iter()
            .any(|family_id| self.family_supports_text(family_id, character_text))
    }

    fn seed_fontique_fallbacks(&mut self, key: FallbackKey, character: char) -> bool {
        let fallback_families = self.find_fallback_families(key.script(), character);
        if fallback_families.is_empty() {
            return false;
        }

        self.font_context
            .collection
            .append_fallbacks(key, fallback_families.into_iter())
    }

    fn find_fallback_families(
        &mut self,
        script: parley::fontique::Script,
        character: char,
    ) -> Vec<FamilyId> {
        let mut character_buffer = [0; 4];
        let character_text = character.encode_utf8(&mut character_buffer);
        let sample_text = script.sample().unwrap_or(character_text);
        let use_sample_text = sample_text != character_text;
        let search_families = Arc::clone(&self.fallback_search_families);

        let mut preferred = Vec::new();
        let mut fallback_only = Vec::new();
        for &family_id in search_families.iter() {
            if !self.family_supports_text(family_id, character_text) {
                continue;
            }

            if use_sample_text && self.family_supports_text(family_id, sample_text) {
                preferred.push(family_id);
            } else {
                fallback_only.push(family_id);
            }
        }

        preferred.extend(fallback_only);
        preferred
    }

    fn family_supports_text(&mut self, family_id: FamilyId, text: &str) -> bool {
        let Some(family) = self.font_context.collection.family(family_id) else {
            return false;
        };

        family.fonts().iter().any(|font| {
            let Some(data) = font.load(Some(&mut self.font_context.source_cache)) else {
                return false;
            };
            let Some(charmap) = font.charmap_index().charmap(data.as_ref()) else {
                return false;
            };

            text.chars()
                .all(|character| charmap.map(character).is_some_and(|glyph_id| glyph_id != 0))
        })
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
            metrics.descent = metrics.descent.max(line.metrics().descent);
            metrics.underline_position =
                metrics.underline_position.max(run_metrics.underline_offset);
            metrics.underline_thickness = metrics
                .underline_thickness
                .max(run_metrics.underline_size.max(1.0));
            metrics.strikeout_position = metrics
                .strikeout_position
                .max(run_metrics.strikethrough_offset);
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
        self.raster_backend.checked_fallbacks.clear();
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
        let fallback_search_families = fallback_search_families(&mut font_context);
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
                fallback_search_families,
                checked_fallbacks: FxHashSet::default(),
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
            rgba_scratch: vec![0; width as usize * height as usize * 4],
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
        self.rgba_scratch
            .resize(width as usize * height as usize * 4, 0);
    }

    fn render_scene(
        &mut self,
        scene: &Scene,
        width: u32,
        height: u32,
        bg: Color,
    ) -> Result<&[u8], String> {
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
        self.rgba_scratch.resize(row_len * height as usize, 0);
        for y in 0..height as usize {
            let src = y * self.padded_bytes_per_row as usize;
            let dst = y * row_len;
            self.rgba_scratch[dst..dst + row_len].copy_from_slice(&mapped[src..src + row_len]);
        }
        drop(mapped);
        self.readback.unmap();
        Ok(&self.rgba_scratch[..row_len * height as usize])
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

fn blit_rgba(
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

fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
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

    push_family(
        &mut families,
        FontFamily::Generic(GenericFamily::UiMonospace),
    );
    push_family(&mut families, FontFamily::Generic(GenericFamily::Monospace));
    push_family(&mut families, FontFamily::Generic(GenericFamily::SystemUi));
    push_family(&mut families, FontFamily::Generic(GenericFamily::Emoji));

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

fn fallback_search_families(font_context: &mut FontContext) -> Arc<[FamilyId]> {
    let mut families = Vec::new();
    let mut seen = FxHashSet::default();

    for generic_family in [
        GenericFamily::UiMonospace,
        GenericFamily::Monospace,
        GenericFamily::SystemUi,
        GenericFamily::Emoji,
    ] {
        for family_id in font_context.collection.generic_families(generic_family) {
            if seen.insert(family_id) {
                families.push(family_id);
            }
        }
    }

    let mut family_names = font_context
        .collection
        .family_names()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    family_names.sort_unstable_by_key(|family_name| family_name_sort_key(family_name));
    family_names.dedup();

    for family_name in family_names {
        let Some(family_id) = font_context.collection.family_id(&family_name) else {
            continue;
        };
        if seen.insert(family_id) {
            families.push(family_id);
        }
    }

    Arc::from(families)
}

fn family_name_sort_key(family_name: &str) -> (bool, String) {
    (
        family_name.starts_with('.'),
        family_name.to_ascii_lowercase(),
    )
}

fn scene_glyph_from_layout(
    cursor_x: &mut f32,
    baseline: f32,
    glyph: parley::layout::Glyph,
) -> Glyph {
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

fn fontique_script_for_char(character: char) -> Option<parley::fontique::Script> {
    let tag = character.script().to_opentype();
    let mut bytes = [
        (tag >> 24) as u8,
        (tag >> 16) as u8,
        (tag >> 8) as u8,
        tag as u8,
    ];
    bytes[0] = bytes[0].to_ascii_uppercase();
    bytes[1] = bytes[1].to_ascii_lowercase();
    bytes[2] = bytes[2].to_ascii_lowercase();
    bytes[3] = bytes[3].to_ascii_lowercase();
    let script = parley::fontique::Script(bytes);
    (!matches!(&script.0, b"Zyyy" | b"Zinh" | b"Zzzz")).then_some(script)
}
