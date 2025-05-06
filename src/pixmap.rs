/// A pixmap with RGBA pixels stored in a flat vector.
#[derive(Debug, Clone)]
pub struct RgbPixmap {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl RgbPixmap {
    /// Creates a new pixmap.
    ///
    /// (new width height color) -> RgbPixmap
    ///
    /// * color : [u8; 3] - RGB pixel
    pub fn new(width: usize, height: usize) -> Self {
        let data = [0, 0, 0].repeat(width * height);
        Self {
            width,
            height,
            data,
        }
    }
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut rgba_data = Vec::with_capacity(self.width * self.height * 4);
        for chunk in self.data.chunks_exact(3) {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            rgba_data.extend_from_slice(&[r, g, b, 255]); // Alpha = 255
        }
        rgba_data
    }

    /// Sets the RGB value of a pixel at (x, y).
    ///
    /// * color : [u8; 3] - RGB pixel
    pub fn put_pixel(&mut self, x: usize, y: usize, color: [u8; 3]) {
        /*  println!(
            "tried to put pixel at {},{} on screen size of {},{}",
            x, y, self.width, self.height
        ); */
        debug_assert!(
            x < self.width && y < self.height,
            "Pixel coordinates out of bounds"
        );
        let index = 3 * (y * self.width + x);
        self.data[index..index + 3].copy_from_slice(&color);
    }

    /// Returns the RGB value of a pixel at (x, y).
    ///
    /// -> [u8; 3] - RGB pixel
    pub fn get_pixel(&self, x: usize, y: usize) -> [u8; 3] {
        debug_assert!(
            x < self.width && y < self.height,
            "Pixel coordinates out of bounds"
        );
        let index = 3 * (y * self.width + x);
        self.data[index..index + 3].try_into().unwrap()
    }

    /// Fills the entire pixmap with the specified RGB color.
    ///
    /// * color : [u8; 3] - RGB pixel
    pub fn fill(&mut self, color: [u8; 3]) {
        for chunk in self.data.chunks_mut(3) {
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
