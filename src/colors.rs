use ratatui::style::Color as RatColor;

///Converts a Ratatui Color into a rgb [u8;3]
pub fn rat_to_rgb(rat_col: &RatColor, is_a_fg: bool) -> [u8; 3] {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                [204, 204, 255] // foreground reset color
            } else {
                [150, 15, 112] // background reset color
            }
        }
        RatColor::Black => [0, 0, 0],
        RatColor::Red => [139, 0, 0],
        RatColor::Green => [0, 100, 0],
        RatColor::Yellow => [255, 215, 0],
        RatColor::Blue => [0, 0, 139],
        RatColor::Magenta => [99, 9, 99],
        RatColor::Cyan => [0, 0, 255],
        RatColor::Gray => [128, 128, 128],
        RatColor::DarkGray => [64, 64, 64],
        RatColor::LightRed => [255, 0, 0],
        RatColor::LightGreen => [0, 255, 0],
        RatColor::LightBlue => [173, 216, 230],
        RatColor::LightYellow => [255, 255, 224],
        RatColor::LightMagenta => [139, 0, 139],
        RatColor::LightCyan => [224, 255, 255],
        RatColor::White => [255, 255, 255],
        RatColor::Indexed(i) => {
            let i = *i as u8;
            [i.wrapping_mul(i), i.wrapping_add(i), i]
        }
        RatColor::Rgb(r, g, b) => [*r, *g, *b],
    }
}

pub fn dim_rgb(color: [u8; 3]) -> [u8; 3] {
    let factor = 77; // 77 ≈ 255 * 0.3
    [
        ((color[0] as u32 * factor + 127) / 255) as u8,
        ((color[1] as u32 * factor + 127) / 255) as u8,
        ((color[2] as u32 * factor + 127) / 255) as u8,
    ]
}
