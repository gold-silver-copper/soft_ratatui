pub use pixmap::RgbPixmap;

/*mod soft_backend;
pub use soft_backend::SoftBackend; */
/*mod ab_glyph;
pub use ab_glyph::SoftBackend; */
mod bdf_backend;
pub use bdf_backend::SoftBackend;

mod colors;

mod pixmap;
