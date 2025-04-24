use cosmic_text::{Attrs, Buffer, Color as CosmicColor, FontSystem, Metrics, Shaping, SwashCache};
use tiny_skia::{Color as SkiaColor, Paint, Pixmap, Rect};

fn main() {
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    let metrics = Metrics::new(14.0, 20.0);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);

    // Set a size for the text buffer, in pixels
    let width = 160;
    let height = 50;
    buffer.set_size(Some(width as f32), Some(height as f32));

    // Set and shape text
    buffer.set_text("Hello, Rust! 🦀\n", &Attrs::new(), Shaping::Advanced);
    buffer.shape_until_scroll(true);

    // Prepare Pixmap to draw into
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(SkiaColor::BLACK); // black background

    let text_color = CosmicColor::rgb(0xFF, 0xFF, 0xFF); // white

    // Draw using tiny-skia
    buffer.draw(&mut swash_cache, text_color, |x, y, w, h, color| {
        if let Some(rect) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
            let mut paint = Paint::default();
            let [r, g, b, a] = color.as_rgba();
            paint.set_color(SkiaColor::from_rgba8(r, g, b, a));
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        }
    });

    // Save to PNG file
    pixmap.save_png("output_tiny_skia.png").unwrap();
}
