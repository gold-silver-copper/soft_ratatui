pub use pixmap::RgbPixmap;

mod embedded_backend;
pub use embedded_backend::SoftBackend;
mod colors;

mod pixmap;

#[cfg(feature = "cozette")]
pub mod cozette;
#[cfg(feature = "scientifica")]
pub mod scientifica;
//pub use cozette;
#[cfg(feature = "unicodefonts")]
pub use embedded_graphics_unicodefonts;
#[cfg(feature = "cozette_hidpi")]
pub mod cozette_hidpi;
