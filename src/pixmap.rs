/// A pixmap with RGBA pixels stored in a flat vector.
#[derive(Debug, Clone)]
pub struct RgbaPixmap {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl RgbaPixmap {
    /// Creates a new pixmap filled with the specified RGBA color.
    ///
    /// (new width height color) -> RgbaPixmap
    ///
    /// * color : [u8; 4] - RGBA pixel
    pub fn new(width: usize, height: usize) -> Self {
        let data = [0, 0, 0, 255].repeat(width * height);
        Self {
            width,
            height,
            data,
        }
    }

    /// Sets the RGBA value of a pixel at (x, y).
    ///
    /// * color : [u8; 4] - RGBA pixel
    pub fn put_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        assert!(
            x < self.width && y < self.height,
            "Pixel coordinates out of bounds"
        );
        let index = 4 * (y * self.width + x);
        self.data[index..index + 4].copy_from_slice(&color);
    }

    /// Returns the RGBA value of a pixel at (x, y).
    ///
    /// -> [u8; 4] - RGBA pixel
    pub fn get_pixel(&self, x: usize, y: usize) -> [u8; 4] {
        assert!(
            x < self.width && y < self.height,
            "Pixel coordinates out of bounds"
        );
        let index = 4 * (y * self.width + x);
        self.data[index..index + 4].try_into().unwrap()
    }

    /// Fills the entire pixmap with the specified RGBA color.
    ///
    /// * color : [u8; 4] - RGBA pixel
    pub fn fill(&mut self, color: [u8; 4]) {
        for chunk in self.data.chunks_mut(4) {
            chunk.copy_from_slice(&color);
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
