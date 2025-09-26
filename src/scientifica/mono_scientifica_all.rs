pub const MONO_SCIENTIFICA_ALL: ::embedded_graphics::mono_font::MonoFont = ::embedded_graphics::mono_font::MonoFont {
    image: ::embedded_graphics::image::ImageRaw::new(
        include_bytes!("mono_scientifica_all.data"),
        256u32,
    ),
    glyph_mapping: &::embedded_graphics::mono_font::mapping::StrGlyphMapping::new(
        "\0 \u{d7ff}\0豈�",
        57053usize,
    ),
    character_size: ::embedded_graphics::geometry::Size::new(16u32, 16u32),
    character_spacing: 0u32,
    baseline: 13u32,
    underline: ::embedded_graphics::mono_font::DecorationDimensions::new(15u32, 1u32),
    strikethrough: ::embedded_graphics::mono_font::DecorationDimensions::new(8u32, 1u32),
};
