pub use pixmap::RgbPixmap;

mod soft_backend;
pub use soft_backend::SoftBackend;
mod colors;

mod pixmap;

/*#[cfg(feature = "cozette")]
pub mod cozette; */

#[cfg(feature = "embedded-graphics")]
mod embedded_backend;
#[cfg(feature = "embedded-graphics")]
pub use embedded_backend::EmbeddedGraphics;

#[cfg(feature = "unicodefonts")]
pub use embedded_graphics_unicodefonts;

#[cfg(feature = "cosmic-text")]
pub use cosmic_backend::CosmicText;
#[cfg(feature = "cosmic-text")]
mod cosmic_backend;
#[cfg(feature = "bdf-parser")]
pub use bdf_backend::Bdf;
#[cfg(feature = "bdf-parser")]
mod bdf_backend;
