pub type Point = euclid::default::Point2D<f32>;
pub type Vec2 = euclid::default::Vector2D<f32>;
pub type Size = euclid::default::Size2D<f32>;
pub type Rect = euclid::default::Box2D<f32>;

pub type Color = [f32; 4];

pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    [r, g, b, 1.0]
}

// NDC = normalized device space. We work with the cpoordinates being (0, 0) in the top left of our
// boxes, the device works with (0, 0) being the center of the paint surface.
pub fn to_ndc(pos: Point, width: f32, height: f32) -> Vec2 {
    Vec2::new(pos.x / width * 2.0 - 1.0, 1.0 - pos.y / height * 2.0)
}
