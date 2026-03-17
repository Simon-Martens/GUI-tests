mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Point, Rect, Size, rgb};
use crate::ui::{AnyElement, Render, Window, quad};

fn main() {
    app::run(Demo);
}

// INFO: Demo is our state struct. It is retained across frames and should own all the state it
// neeeds to render itself.
struct Demo;

impl Render for Demo {
    fn render(&mut self, _window: &mut Window<'_>) -> AnyElement {
        quad(
            Rect::from_origin_and_size(Point::new(36.0, 72.0), Size::new(96.0, 56.0)),
            rgb(0.82, 0.29, 0.24),
        )
    }
}
