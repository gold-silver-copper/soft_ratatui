//! Parley shaping with CPU glyph rasterization through Swash.

use std::borrow::Cow;
use std::sync::Arc;

use crate::SoftBackend;
use crate::colors::{dim_rgb, rat_to_rgb};
use crate::pixmap::RgbPixmap;
use crate::soft_backend::{BlinkConfig, CursorConfig, RasterBackend};
use parley::fontique::Blob;
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
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source, StrikeWith, image::Content, image::Image};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct LayoutKey {
    character: char,
    bold: bool,
    italic: bool,
}

/// Parley text shaping with CPU glyph rasterization through Swash.
pub struct ParleySwash {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    family_stack: Arc<[FontFamily<'static>]>,
    font_size: f32,
    cell_height: usize,
    baseline: f32,
    underline_position: f32,
    strikeout_position: f32,
    cache: FxHashMap<LayoutKey, Arc<Layout<()>>>,
    scale_context: ScaleContext,
}

impl RasterBackend for ParleySwash {
    fn draw_cell(
        &mut self,
        x: u16,
        y: u16,
        cell: &Cell,
        always_redraw_list: &mut rustc_hash::FxHashSet<(u16, u16)>,
        blinking_fast: bool,
        blinking_slow: bool,
        char_width: usize,
        char_height: usize,
        pixmap: &mut RgbPixmap,
    ) {
        let mut fg = cell.fg;
        let bg = cell.bg;
        if cell.modifier.contains(Modifier::HIDDEN) {
            fg = bg;
        }
        let (mut fg, mut bg) = if cell.modifier.contains(Modifier::REVERSED) {
            (rat_to_rgb(&bg, false), rat_to_rgb(&fg, true))
        } else {
            (rat_to_rgb(&fg, true), rat_to_rgb(&bg, false))
        };
        if cell.modifier.contains(Modifier::DIM) {
            (fg, bg) = (dim_rgb(fg), dim_rgb(bg));
        }
        if cell.modifier.contains(Modifier::SLOW_BLINK) {
            always_redraw_list.insert((x, y));
            if blinking_slow {
                fg = bg;
            }
        }
        if cell.modifier.contains(Modifier::RAPID_BLINK) {
            always_redraw_list.insert((x, y));
            if blinking_fast {
                fg = bg;
            }
        }

        let origin_x = x as usize * char_width;
        let origin_y = y as usize * char_height;
        pixmap.fill_rect(origin_x, origin_y, char_width, char_height, bg);
        if cell.symbol().chars().any(|c| !c.is_whitespace()) {
            let layout = self.shape_cell(
                cell.symbol(),
                cell.modifier.contains(Modifier::BOLD),
                cell.modifier.contains(Modifier::ITALIC),
            );
            self.paint_layout(pixmap, &layout, origin_x as f32, origin_y as f32, fg);
        }

        if cell.modifier.contains(Modifier::UNDERLINED) {
            let y = (origin_y as f32 + self.baseline - self.underline_position)
                .round()
                .max(origin_y as f32) as usize;
            pixmap.fill_rect(origin_x, y, char_width, 1, fg);
        }
        if cell.modifier.contains(Modifier::CROSSED_OUT) {
            let y = (origin_y as f32 + self.baseline - self.strikeout_position)
                .round()
                .max(origin_y as f32) as usize;
            pixmap.fill_rect(origin_x, y, char_width, 1, fg);
        }
    }
}

impl ParleySwash {
    fn shape_cell(&mut self, text: &str, bold: bool, italic: bool) -> Arc<Layout<()>> {
        let mut chars = text.chars();
        if let (Some(character), None) = (chars.next(), chars.next()) {
            let key = LayoutKey {
                character,
                bold,
                italic,
            };
            if let Some(layout) = self.cache.get(&key) {
                return Arc::clone(layout);
            }
            let text = character.to_string();
            let layout = self.shape(&text, bold, italic);
            self.cache.insert(key, Arc::clone(&layout));
            layout
        } else {
            self.shape(text, bold, italic)
        }
    }

    fn shape(&mut self, text: &str, bold: bool, italic: bool) -> Arc<Layout<()>> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(FontStack::from(&self.family_stack[..]));
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::FontWeight(if bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        }));
        builder.push_default(StyleProperty::FontStyle(if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        builder.push_default(LineHeight::Absolute(self.cell_height as f32));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());
        Arc::new(layout)
    }

    fn paint_layout(
        &mut self,
        pixmap: &mut RgbPixmap,
        layout: &Layout<()>,
        origin_x: f32,
        origin_y: f32,
        foreground: [u8; 3],
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
                    let Some(image) = Render::new(&[
                        Source::ColorOutline(0),
                        Source::ColorBitmap(StrikeWith::BestFit),
                        Source::Outline,
                    ])
                    .render(&mut scaler, glyph.id as u16) else {
                        continue;
                    };
                    blit_glyph(pixmap, &image, glyph_x, glyph_y, foreground);
                }
            }
        }
    }
}

impl SoftBackend<ParleySwash> {
    /// Creates a software backend with Parley shaping and Swash CPU rasterization.
    pub fn new(width: u16, height: u16, font_size: i32, font_data: &[u8]) -> Self {
        let mut font_context = FontContext::new();
        let existing_families = font_context
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
            .filter(|name| !existing_families.contains(*name))
            .map(|name| FontFamily::Named(Cow::Owned(name.to_owned())))
            .collect::<Vec<_>>();
        if families.is_empty() {
            families = font_context
                .collection
                .family_names()
                .map(|name| FontFamily::Named(Cow::Owned(name.to_owned())))
                .collect();
        }
        families.push(FontFamily::Generic(GenericFamily::UiMonospace));
        families.push(FontFamily::Generic(GenericFamily::Monospace));
        let family_stack: Arc<[FontFamily<'static>]> = Arc::from(families);
        let mut layout_context = LayoutContext::default();
        let size = font_size as f32;
        let mut builder = layout_context.ranged_builder(&mut font_context, "M", 1.0, true);
        builder.push_default(FontStack::from(&family_stack[..]));
        builder.push_default(StyleProperty::FontSize(size));
        let mut metrics_layout = builder.build("M");
        metrics_layout.break_all_lines(None);
        let line = metrics_layout.lines().next().expect("font metrics line");
        let run_metrics = line
            .items()
            .find_map(|item| match item {
                PositionedLayoutItem::GlyphRun(run) => Some(*run.run().metrics()),
                _ => None,
            })
            .unwrap_or_default();
        let cell_width = metrics_layout.full_width().ceil().max(1.0) as usize;
        let cell_height = line.metrics().line_height.ceil().max(1.0) as usize;
        let baseline = line.metrics().baseline;
        let mut backend = Self {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: false,
            cursor_pos: (0, 0),
            cursor_config: CursorConfig::default(),
            raster_backend: ParleySwash {
                font_context,
                layout_context,
                family_stack,
                font_size: size,
                cell_height,
                baseline,
                underline_position: run_metrics.underline_offset,
                strikeout_position: run_metrics.strikethrough_offset,
                cache: FxHashMap::default(),
                scale_context: ScaleContext::new(),
            },
            rgb_pixmap: RgbPixmap::new(cell_width * width as usize, cell_height * height as usize),
            char_width: cell_width,
            char_height: cell_height,
            frame_count: 0,
            blink_config: BlinkConfig::default(),
            always_redraw_list: FxHashSet::default(),
            rendered_cursor: None,
        };
        _ = backend.clear();
        backend
    }

    /// Sets a new font size and recreates the output pixmap.
    pub fn set_font_size(&mut self, font_size: i32) {
        self.raster_backend.font_size = font_size as f32;
        self.raster_backend.cache.clear();
        let layout = self.raster_backend.shape("M", false, false);
        let line = layout.lines().next().expect("font metrics line");
        self.char_width = layout.full_width().ceil().max(1.0) as usize;
        self.char_height = line.metrics().line_height.ceil().max(1.0) as usize;
        self.raster_backend.cell_height = self.char_height;
        self.raster_backend.baseline = line.metrics().baseline;
        self.rgb_pixmap = RgbPixmap::new(
            self.char_width * self.buffer.area.width as usize,
            self.char_height * self.buffer.area.height as usize,
        );
        self.redraw();
    }
}

fn blit_glyph(
    pixmap: &mut RgbPixmap,
    image: &Image,
    baseline_x: f32,
    baseline_y: f32,
    foreground: [u8; 3],
) {
    let left = baseline_x.floor() as i32 + image.placement.left;
    let top = baseline_y.floor() as i32 - image.placement.top;
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let channels = if image.content == Content::Mask { 1 } else { 4 };
    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * channels;
            let (source, alpha) = match image.content {
                Content::Mask => (foreground, image.data[offset]),
                Content::SubpixelMask => {
                    let alpha = ((image.data[offset] as u16
                        + image.data[offset + 1] as u16
                        + image.data[offset + 2] as u16)
                        / 3) as u8;
                    (foreground, alpha)
                }
                Content::Color => (
                    [
                        image.data[offset],
                        image.data[offset + 1],
                        image.data[offset + 2],
                    ],
                    image.data[offset + 3],
                ),
            };
            let px = left + x as i32;
            let py = top + y as i32;
            if alpha == 0 || px < 0 || py < 0 {
                continue;
            }
            let (px, py) = (px as usize, py as usize);
            if px >= pixmap.width || py >= pixmap.height {
                continue;
            }
            let background = pixmap.get_pixel(px, py);
            let a = alpha as u16;
            pixmap.put_pixel(
                px,
                py,
                [
                    ((source[0] as u16 * a + background[0] as u16 * (255 - a) + 127) / 255) as u8,
                    ((source[1] as u16 * a + background[1] as u16 * (255 - a) + 127) / 255) as u8,
                    ((source[2] as u16 * a + background[2] as u16 * (255 - a) + 127) / 255) as u8,
                ],
            );
        }
    }
}
