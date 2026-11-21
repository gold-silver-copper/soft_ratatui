//! Parley + Swash CPU rasterization backend for [`SoftBackend`].

use std::borrow::Cow;
use std::sync::Arc;

use crate::SoftBackend;
use crate::colors::*;
use crate::pixmap::RgbPixmap;
use crate::soft_backend::RasterBackend;
use crate::soft_backend::{BlinkConfig, CursorConfig};
use parley::fontique::{Blob, FallbackKey, FamilyId};
use parley::layout::PositionedLayoutItem;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontStack, FontStyle, FontWeight,
    GenericFamily, Layout, LayoutContext, LineHeight, StyleProperty,
};
use ratatui_core::backend::Backend;
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::layout::Rect;
use ratatui_core::style::Modifier;
use rustc_hash::{FxHashMap, FxHashSet};
use swash::scale::{Render, ScaleContext, Source, StrikeWith, image::Content, image::Image};
use swash::{FontRef, text::Codepoint as _};
use unicode_width::UnicodeWidthStr;

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

#[derive(Clone, Copy, Debug)]
struct ResolvedCellStyle {
    fg_color: [u8; 3],
    bg_color: [u8; 3],
    display_width: usize,
}

/// Parley text shaping with CPU glyph rasterization through Swash.
pub struct ParleySwash {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    family_stack: Arc<[FontFamily<'static>]>,
    font_size: f32,
    metrics: TextMetrics,
    locale: Option<String>,
    fallback_search_families: Arc<[FamilyId]>,
    checked_fallbacks: FxHashSet<(FallbackKey, char)>,
    cache: FxHashMap<LayoutKey, Arc<Layout<()>>>,
    scale_context: ScaleContext,
}

impl RasterBackend for ParleySwash {
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
        pixmap: &mut RgbPixmap,
    ) {
        always_redraw_list.clear();

        // Paint backgrounds first so wide cells can cover their trailing cells.
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                let style =
                    self.resolve_buffer_cell_style(buffer, x, y, blinking_fast, blinking_slow);
                if cell
                    .modifier
                    .intersects(Modifier::SLOW_BLINK | Modifier::RAPID_BLINK)
                {
                    always_redraw_list.insert((x, y));
                }
                pixmap.fill_rect(
                    x as usize * char_width,
                    y as usize * char_height,
                    char_width * style.display_width,
                    char_height,
                    style.bg_color,
                );
            }
        }

        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                let style =
                    self.resolve_buffer_cell_style(buffer, x, y, blinking_fast, blinking_slow);
                let origin_x = (x as usize * char_width) as f32;
                let origin_y = (y as usize * char_height) as f32;
                if Self::should_shape_text(cell) {
                    let layout = self.shape_cell_text(
                        cell.symbol(),
                        font_variant_from_style(
                            cell.modifier.contains(Modifier::BOLD),
                            cell.modifier.contains(Modifier::ITALIC),
                        ),
                    );
                    self.paint_layout(pixmap, &layout, origin_x, origin_y, style.fg_color);
                }
                self.paint_decorations(
                    pixmap,
                    origin_x as usize,
                    origin_y as usize,
                    char_width * style.display_width,
                    style.fg_color,
                    cell,
                );
            }
        }
    }

    fn draw_cell(
        &mut self,
        x: u16,
        y: u16,
        cell: &Cell,
        always_redraw_list: &mut FxHashSet<(u16, u16)>,
        blinking_fast: bool,
        blinking_slow: bool,
        char_width: usize,
        char_height: usize,
        pixmap: &mut RgbPixmap,
    ) {
        let style = self.resolve_cell_style(cell, blinking_fast, blinking_slow);
        if cell
            .modifier
            .intersects(Modifier::SLOW_BLINK | Modifier::RAPID_BLINK)
        {
            always_redraw_list.insert((x, y));
        }
        let begin_x = x as usize * char_width;
        let begin_y = y as usize * char_height;
        pixmap.fill_rect(
            begin_x,
            begin_y,
            char_width * style.display_width,
            char_height,
            style.bg_color,
        );
        if Self::should_shape_text(cell) {
            let layout = self.shape_cell_text(
                cell.symbol(),
                font_variant_from_style(
                    cell.modifier.contains(Modifier::BOLD),
                    cell.modifier.contains(Modifier::ITALIC),
                ),
            );
            self.paint_layout(
                pixmap,
                &layout,
                begin_x as f32,
                begin_y as f32,
                style.fg_color,
            );
        }
        self.paint_decorations(
            pixmap,
            begin_x,
            begin_y,
            char_width * style.display_width,
            style.fg_color,
            cell,
        );
    }
}

impl ParleySwash {
    fn should_shape_text(cell: &Cell) -> bool {
        cell.symbol()
            .chars()
            .any(|character| !character.is_whitespace())
    }

    fn cell_owner_style(
        &self,
        buffer: &Buffer,
        x: u16,
        y: u16,
        blinking_fast: bool,
        blinking_slow: bool,
    ) -> Option<ResolvedCellStyle> {
        if x == 0 {
            return None;
        }
        let previous = &buffer[(x - 1, y)];
        (UnicodeWidthStr::width(previous.symbol()) > 1)
            .then(|| self.resolve_cell_style(previous, blinking_fast, blinking_slow))
    }

    fn resolve_buffer_cell_style(
        &self,
        buffer: &Buffer,
        x: u16,
        y: u16,
        blinking_fast: bool,
        blinking_slow: bool,
    ) -> ResolvedCellStyle {
        let cell = &buffer[(x, y)];
        let mut style = self.resolve_cell_style(cell, blinking_fast, blinking_slow);
        if cell.symbol() == " "
            && matches!(cell.bg, ratatui_core::style::Color::Reset)
            && let Some(owner) = self.cell_owner_style(buffer, x, y, blinking_fast, blinking_slow)
        {
            style.fg_color = owner.fg_color;
            style.bg_color = owner.bg_color;
            style.display_width = 1;
        }
        style
    }

    fn resolve_cell_style(
        &self,
        cell: &Cell,
        blinking_fast: bool,
        blinking_slow: bool,
    ) -> ResolvedCellStyle {
        let mut fg = cell.fg;
        let bg = cell.bg;
        if cell.modifier.contains(Modifier::HIDDEN) {
            fg = bg;
        }
        let (mut fg_color, mut bg_color) = if cell.modifier.contains(Modifier::REVERSED) {
            (rat_to_rgb(&bg, false), rat_to_rgb(&fg, true))
        } else {
            (rat_to_rgb(&fg, true), rat_to_rgb(&bg, false))
        };
        if cell.modifier.contains(Modifier::DIM) {
            (fg_color, bg_color) = (dim_rgb(fg_color), dim_rgb(bg_color));
        }
        if cell.modifier.contains(Modifier::SLOW_BLINK) && blinking_slow {
            fg_color = bg_color;
        }
        if cell.modifier.contains(Modifier::RAPID_BLINK) && blinking_fast {
            fg_color = bg_color;
        }
        ResolvedCellStyle {
            fg_color,
            bg_color,
            display_width: UnicodeWidthStr::width(cell.symbol()).clamp(1, 2),
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
        let mut buf = [0; 4];
        let text = character.encode_utf8(&mut buf);
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
        let (style, weight) = font_style(variant);
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(FontStack::from(&self.family_stack[..]));
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::FontStyle(style));
        builder.push_default(StyleProperty::FontWeight(weight));
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
        &mut self,
        pixmap: &mut RgbPixmap,
        layout: &Layout<()>,
        origin_x: f32,
        origin_y: f32,
        color: [u8; 3],
    ) {
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let Some(font_ref) = FontRef::from_index(font.data.as_ref(), font.index as usize)
                else {
                    continue;
                };
                let mut scaler = self
                    .scale_context
                    .builder(font_ref)
                    .size(run.font_size())
                    .hint(true)
                    .normalized_coords(run.normalized_coords().iter().copied())
                    .build();
                let mut pen_x = glyph_run.offset();
                let baseline = glyph_run.baseline();
                for glyph in glyph_run.glyphs() {
                    let glyph_x = origin_x + pen_x + glyph.x;
                    let glyph_y = origin_y + baseline - glyph.y;
                    pen_x += glyph.advance;
                    if let Some(image) = Render::new(&[
                        Source::ColorOutline(0),
                        Source::ColorBitmap(StrikeWith::BestFit),
                        Source::Outline,
                    ])
                    .render(&mut scaler, glyph.id as u16)
                    {
                        blit_glyph(pixmap, &image, glyph_x, glyph_y, color);
                    }
                }
            }
        }
    }

    fn paint_decorations(
        &self,
        pixmap: &mut RgbPixmap,
        x: usize,
        y: usize,
        width: usize,
        color: [u8; 3],
        cell: &Cell,
    ) {
        if cell.modifier.contains(Modifier::UNDERLINED) {
            let (dy, height) = self.decoration_geometry(y, true);
            pixmap.fill_rect(x, dy, width, height, color);
        }
        if cell.modifier.contains(Modifier::CROSSED_OUT) {
            let (dy, height) = self.decoration_geometry(y, false);
            pixmap.fill_rect(x, dy, width, height, color);
        }
    }

    fn decoration_geometry(&self, begin_y: usize, underline: bool) -> (usize, usize) {
        let (position, thickness) = if underline {
            (
                self.metrics.underline_position,
                self.metrics.underline_thickness.max(1.0),
            )
        } else {
            (
                self.metrics.strikeout_position,
                self.metrics.strikeout_thickness.max(1.0),
            )
        };
        let h = self.metrics.cell_height.max(1.0);
        let dy = ((begin_y as f32 + self.metrics.baseline - position) - thickness / 2.0)
            .round()
            .max(begin_y as f32)
            .min(begin_y as f32 + h - thickness.max(1.0));
        (dy as usize, thickness.ceil().max(1.0) as usize)
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

impl SoftBackend<ParleySwash> {
    /// Sets a new font size for the terminal image.
    pub fn set_font_size(&mut self, font_size: i32) {
        self.raster_backend.font_size = font_size as f32;
        self.raster_backend.metrics = ParleySwash::measure_metrics(
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
        let metrics = ParleySwash::measure_metrics(
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
            raster_backend: ParleySwash {
                font_context,
                layout_context,
                family_stack,
                font_size: font_size as f32,
                metrics,
                locale,
                fallback_search_families,
                checked_fallbacks: FxHashSet::default(),
                cache: FxHashMap::default(),
                scale_context: ScaleContext::new(),
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

fn blit_glyph(
    pixmap: &mut RgbPixmap,
    image: &Image,
    baseline_x: f32,
    baseline_y: f32,
    color: [u8; 3],
) {
    let x0 = baseline_x.floor() as i32 + image.placement.left;
    let y0 = baseline_y.floor() as i32 - image.placement.top;
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let channels = match image.content {
        Content::Mask => 1,
        _ => 4,
    };
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * channels;
            let (src, alpha) = match image.content {
                Content::Mask => (color, image.data[index]),
                Content::SubpixelMask => {
                    let a = ((image.data[index] as u16
                        + image.data[index + 1] as u16
                        + image.data[index + 2] as u16)
                        / 3) as u8;
                    (color, a)
                }
                Content::Color => (
                    [
                        image.data[index],
                        image.data[index + 1],
                        image.data[index + 2],
                    ],
                    image.data[index + 3],
                ),
            };
            if alpha == 0 {
                continue;
            }
            let px = x0 + x as i32;
            let py = y0 + y as i32;
            if px < 0 || py < 0 {
                continue;
            }
            let (px, py) = (px as usize, py as usize);
            if px >= pixmap.width || py >= pixmap.height {
                continue;
            }
            let bg = pixmap.get_pixel(px, py);
            let a = alpha as u16;
            let blended = [
                ((src[0] as u16 * a + bg[0] as u16 * (255 - a) + 127) / 255) as u8,
                ((src[1] as u16 * a + bg[1] as u16 * (255 - a) + 127) / 255) as u8,
                ((src[2] as u16 * a + bg[2] as u16 * (255 - a) + 127) / 255) as u8,
            ];
            pixmap.put_pixel(px, py, blended);
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
