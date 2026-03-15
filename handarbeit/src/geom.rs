pub type Color = [f32; 4];

pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    [r, g, b, 1.0]
}

pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    [r, g, b, a]
}

#[derive(Clone, Copy, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }
}

// Screen style coordiantes: top left is (0, 0) x -> right, y -> down
// Normalized deive coordintes/clip space: center is ~(0, 0)
// x -> -1 to 1 (left to right), y -> -1 to 1 (bottom to top)
pub fn to_ndc(pos: Vec2, width: f32, height: f32) -> Vec2 {
    Vec2::new(pos.x / width * 2.0 - 1.0, 1.0 - pos.y / height * 2.0)
}
