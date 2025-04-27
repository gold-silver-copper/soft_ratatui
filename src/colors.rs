use ratatui::style::Color as RatColor;

pub fn rat_to_rgba(rat_col: &RatColor, is_a_fg: bool) -> [u8; 4] {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                [204, 204, 255, 255] // foreground reset color
            } else {
                [15, 15, 112, 255] // background reset color
            }
        }
        RatColor::Black => [0, 0, 0, 255],
        RatColor::Red => [139, 0, 0, 255],
        RatColor::Green => [0, 100, 0, 255],
        RatColor::Yellow => [255, 215, 0, 255],
        RatColor::Blue => [0, 0, 139, 255],
        RatColor::Magenta => [99, 9, 99, 255],
        RatColor::Cyan => [0, 0, 255, 255],
        RatColor::Gray => [128, 128, 128, 255],
        RatColor::DarkGray => [64, 64, 64, 255],
        RatColor::LightRed => [255, 0, 0, 255],
        RatColor::LightGreen => [0, 255, 0, 255],
        RatColor::LightBlue => [173, 216, 230, 255],
        RatColor::LightYellow => [255, 255, 224, 255],
        RatColor::LightMagenta => [139, 0, 139, 255],
        RatColor::LightCyan => [224, 255, 255, 255],
        RatColor::White => [255, 255, 255, 255],
        RatColor::Indexed(i) => {
            let i = *i as u8;
            [i.wrapping_mul(i), i.wrapping_add(i), i, 255]
        }
        RatColor::Rgb(r, g, b) => [*r, *g, *b, 255],
    }
}
