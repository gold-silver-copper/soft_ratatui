use fontdue::Font;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Transform};

// Load a font (you can use any TTF or OTF font you have)
static FONT_DATA: &[u8] = include_bytes!("../assets/iosevka.ttf");

fn main() {
    // Load the font
    let font =
        Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).expect("Failed to load font");

    // Create a canvas
    let mut pixmap = Pixmap::new(512, 128).expect("Failed to create pixmap");
    pixmap.fill(Color::WHITE);

    // Text you want to render
    let text = "Hello█ █ tiny-skia!";

    // Starting point
    let mut x = 0.0;
    let y = 0.0;

    // Font size
    let font_size = 48.0;

    for ch in text.chars() {
        // Rasterize each character
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        println!("{ch} : {metrics:#?}");

        // Draw the glyph bitmap onto the pixmap
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col] as f32 / 255.0;
                if alpha > 0.0 {
                    pixmap.fill_rect(
                        tiny_skia::Rect::from_xywh(
                            x + (metrics.bounds.xmin) + (col as f32),
                            y + (metrics.bounds.ymin) + (row as f32),
                            1.0,
                            1.0,
                        )
                        .unwrap(),
                        &tiny_skia::Paint {
                            shader: tiny_skia::Shader::SolidColor(Color::BLACK),
                            anti_alias: false,
                            ..Default::default()
                        },
                        Transform::identity(),
                        None, //Some(alpha),
                    );
                }
            }
        }

        // Move x to next glyph
        x += metrics.advance_width;
    }

    // Save the pixmap to a PNG file
    pixmap.save_png("output.png").expect("Failed to save PNG");
}
