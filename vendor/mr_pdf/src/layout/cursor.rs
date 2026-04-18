pub struct Cursor {
    pub x: f64,
    pub y: f64,
}

impl Cursor {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
