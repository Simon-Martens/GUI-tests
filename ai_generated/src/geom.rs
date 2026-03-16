pub use glam::Vec2;

pub type Color = [f32; 4];

pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    [r, g, b, 1.0]
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

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn intersect(self, other: Self) -> Option<Self> {
        let min = Vec2::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y));
        let max = Vec2::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y));
        (min.x < max.x && min.y < max.y).then_some(Self { min, max })
    }

    pub fn contains(self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

pub fn to_ndc(pos: Vec2, width: f32, height: f32) -> Vec2 {
    Vec2::new(pos.x / width * 2.0 - 1.0, 1.0 - pos.y / height * 2.0)
}
