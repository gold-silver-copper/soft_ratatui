use cosmic_text::Color as CosmicColor;

use ratatui::style::Color as RatColor;
use tiny_skia::{Color as SkiaColor, ColorU8 as SkiaColorU8};

/// Convert a ratatui color to a tiny-skia SkiaColor
pub fn rat_to_skia_color(rat_col: &RatColor, is_a_fg: bool) -> SkiaColor {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                SkiaColor::from_rgba8(204, 204, 255, 255)
            } else {
                SkiaColor::from_rgba8(15, 15, 112, 255)
            }
        }
        RatColor::Black => SkiaColor::from_rgba8(0, 0, 0, 255),
        RatColor::Red => SkiaColor::from_rgba8(139, 0, 0, 255),
        RatColor::Green => SkiaColor::from_rgba8(0, 100, 0, 255),
        RatColor::Yellow => SkiaColor::from_rgba8(255, 215, 0, 255),
        RatColor::Blue => SkiaColor::from_rgba8(0, 0, 139, 255),
        RatColor::Magenta => SkiaColor::from_rgba8(99, 9, 99, 255),
        RatColor::Cyan => SkiaColor::from_rgba8(0, 0, 255, 255),
        RatColor::Gray => SkiaColor::from_rgba8(128, 128, 128, 255),
        RatColor::DarkGray => SkiaColor::from_rgba8(64, 64, 64, 255),
        RatColor::LightRed => SkiaColor::from_rgba8(255, 0, 0, 255),
        RatColor::LightGreen => SkiaColor::from_rgba8(0, 255, 0, 255),
        RatColor::LightBlue => SkiaColor::from_rgba8(173, 216, 230, 255),
        RatColor::LightYellow => SkiaColor::from_rgba8(255, 255, 224, 255),
        RatColor::LightMagenta => SkiaColor::from_rgba8(139, 0, 139, 255),
        RatColor::LightCyan => SkiaColor::from_rgba8(224, 255, 255, 255),
        RatColor::White => SkiaColor::from_rgba8(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            // You can customize this mapping, or use a fixed 256-color palette lookup
            let i = *i as u8;
            SkiaColor::from_rgba8(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => SkiaColor::from_rgba8(*r, *g, *b, 255),
    }
}

pub fn rat_to_skia_coloru8(rat_col: &RatColor, is_a_fg: bool) -> SkiaColorU8 {
    match rat_col {
        RatColor::Reset => {
            if is_a_fg {
                SkiaColorU8::from_rgba(204, 204, 255, 255)
            } else {
                SkiaColorU8::from_rgba(15, 15, 112, 255)
            }
        }
        RatColor::Black => SkiaColorU8::from_rgba(0, 0, 0, 255),
        RatColor::Red => SkiaColorU8::from_rgba(139, 0, 0, 255),
        RatColor::Green => SkiaColorU8::from_rgba(0, 100, 0, 255),
        RatColor::Yellow => SkiaColorU8::from_rgba(255, 215, 0, 255),
        RatColor::Blue => SkiaColorU8::from_rgba(0, 0, 139, 255),
        RatColor::Magenta => SkiaColorU8::from_rgba(99, 9, 99, 255),
        RatColor::Cyan => SkiaColorU8::from_rgba(0, 0, 255, 255),
        RatColor::Gray => SkiaColorU8::from_rgba(128, 128, 128, 255),
        RatColor::DarkGray => SkiaColorU8::from_rgba(64, 64, 64, 255),
        RatColor::LightRed => SkiaColorU8::from_rgba(255, 0, 0, 255),
        RatColor::LightGreen => SkiaColorU8::from_rgba(0, 255, 0, 255),
        RatColor::LightBlue => SkiaColorU8::from_rgba(173, 216, 230, 255),
        RatColor::LightYellow => SkiaColorU8::from_rgba(255, 255, 224, 255),
        RatColor::LightMagenta => SkiaColorU8::from_rgba(139, 0, 139, 255),
        RatColor::LightCyan => SkiaColorU8::from_rgba(224, 255, 255, 255),
        RatColor::White => SkiaColorU8::from_rgba(255, 255, 255, 255),
        RatColor::Indexed(i) => {
            // You can customize this mapping, or use a fixed 256-color palette lookup
            let i = *i as u8;
            SkiaColorU8::from_rgba(i.wrapping_mul(i), i.wrapping_add(i), i, 255)
        }
        RatColor::Rgb(r, g, b) => SkiaColorU8::from_rgba(*r, *g, *b, 255),
    }
}

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
