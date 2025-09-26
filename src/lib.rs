pub use pixmap::RgbPixmap;

mod embedded_backend;
pub use embedded_backend::SoftBackend;
mod colors;

mod pixmap;

/*#[cfg(feature = "cozette")]
pub mod cozette; */

#[cfg(feature = "embedded-graphics")]
pub use embedded_backend::EmbeddedGraphics;
#[cfg(feature = "unicodefonts")]
pub use embedded_graphics_unicodefonts;
