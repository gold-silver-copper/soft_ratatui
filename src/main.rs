use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use image::{ImageBuffer, Rgba};
fn main() {
    // A FontSystem provides access to detected system fonts, create one per application
    let mut font_system = FontSystem::new();

    // A SwashCache stores rasterized glyphs, create one per application
    let mut swash_cache = SwashCache::new();

    // Text metrics indicate the font size and line height of a buffer
    let metrics = Metrics::new(14.0, 20.0);

    // A Buffer provides shaping and layout for a UTF-8 string, create one per text widget
    let mut buffer = Buffer::new(&mut font_system, metrics);

    // Borrow buffer together with the font system for more convenient method calls
    let mut buffer = buffer.borrow_with(&mut font_system);

    // Set a size for the text buffer, in pixels
    buffer.set_size(Some(80.0), Some(25.0));

    // Attributes indicate what font to choose
    let attrs = Attrs::new();

    // Add some text!
    buffer.set_text(
        "Hello,正正正正正正正正正正正文)正文)正文) Rust! 🦀 正文)\n",
        &attrs,
        Shaping::Advanced,
    );

    // Perform shaping as desired
    buffer.shape_until_scroll(true);

    // Inspect the output runs
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            println!("{:#?}", glyph);
        }
    }

    // Create a default text color
    let text_color = Color::rgb(0b11111, 0xFF, 0xFF);
    let width = 500;
    let height = 500;
    let mut image = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    // Draw the buffer (for performance, instead use SwashCache directly)
    buffer.draw(&mut swash_cache, text_color, |x, y, w, h, color| {
        // Fill in your code here for drawing rectangles
        let rgba = color.as_rgba();
        for dx in 0..w {
            for dy in 0..h {
                let px = x + dx as i32;
                let py = y + dy as i32;
                if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                    image.put_pixel(px as u32, py as u32, Rgba(rgba));
                }
            }
        }
    });
    image.save("text_output.png").unwrap();
}
