/*
pub fn rat_to_cosmic_color(rat_col: &RatColor, is_a_fg: bool) -> CosmicColor {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                CosmicColor::rgba(204, 204, 255, 255)
            } else {
                CosmicColor::rgba(15, 15, 112, 255)
            }
        }
        RatColor::Black => CosmicColor::rgba(0, 0, 0, 255),
        RatColor::Red => CosmicColor::rgba(139, 0, 0, 255),
        RatColor::Green => CosmicColor::rgba(0, 100, 0, 255),
        RatColor::Yellow => CosmicColor::rgba(255, 215, 0, 255),
        RatColor::Blue => CosmicColor::rgba(0, 0, 139, 255),
        RatColor::Magenta => CosmicColor::rgba(99, 9, 99, 255),
        RatColor::Cyan => CosmicColor::rgba(0, 0, 255, 255),
        RatColor::Gray => CosmicColor::rgba(128, 128, 128, 255),
        RatColor::DarkGray => CosmicColor::rgba(64, 64, 64, 255),
        RatColor::LightRed => CosmicColor::rgba(255, 0, 0, 255),
        RatColor::LightGreen => CosmicColor::rgba(0, 255, 0, 255),
        RatColor::LightBlue => CosmicColor::rgba(173, 216, 230, 255),
        RatColor::LightYellow => CosmicColor::rgba(255, 255, 224, 255),
        RatColor::LightMagenta => CosmicColor::rgba(139, 0, 139, 255),
        RatColor::LightCyan => CosmicColor::rgba(224, 255, 255, 255),
        RatColor::White => CosmicColor::rgba(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            let i = *i as u8;
            CosmicColor::rgba(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => CosmicColor::rgba(*r, *g, *b, 255),
    }
}
 */

/// Blend two RGBA colors using alpha compositing.
///
/// (fg over bg) -> resulting RGB
///
/// * `fg` - [R, G, B, A] foreground color
/// * `bg` - [R, G, B, A] background color
///
/// Returns: blended color as [u8; 4]
pub fn blend_rgba(fg: [u8; 4], bg: [u8; 4]) -> [u8; 3] {
    let fg_a = fg[3] as f32 / 255.0;
    let bg_a = bg[3] as f32 / 255.0;
    let out_a = fg_a + bg_a * (1.0 - fg_a);

    let blend_channel =
        |f: u8, b: u8| ((f as f32 * fg_a + b as f32 * bg_a * (1.0 - fg_a)) / out_a).round() as u8;

    if out_a == 0.0 {
        [0, 0, 0]
    } else {
        [
            blend_channel(fg[0], bg[0]),
            blend_channel(fg[1], bg[1]),
            blend_channel(fg[2], bg[2]),
        ]
    }
}
