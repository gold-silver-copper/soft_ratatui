use embedded_graphics_unicodefonts::atlas::FontAtlas;

/// **Danger**: leaking [`FontAtlas<'static>`] for the lifetime of the program
pub fn mono_scientifica_all_atlas() -> ::embedded_graphics::mono_font::MonoFont<'static> {
    let atlas = FontAtlas::from_mapping_str(1790, "\0 \u{d7ff}\0豈�").leak();

    ::embedded_graphics::mono_font::MonoFont {
        image: ::embedded_graphics::image::ImageRaw::new(
            include_bytes!("mono_scientifica_all.data"),
            256u32,
        ),
        glyph_mapping: atlas,
        character_size: ::embedded_graphics::geometry::Size::new(16u32, 16u32),
        character_spacing: 0u32,
        baseline: 13u32,
        underline: ::embedded_graphics::mono_font::DecorationDimensions::new(15u32, 1u32),
        strikethrough: ::embedded_graphics::mono_font::DecorationDimensions::new(8u32, 1u32),
    }
}
