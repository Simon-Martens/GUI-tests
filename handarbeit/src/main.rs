mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Point, Rect, Size, rgb};
use crate::ui::{AnyElement, IntoElement, Render, Window, quad};

fn main() {
    app::run(Demo);
}

// INFO: Demo is our state struct. It is retained across frames and should own all the state it
// neeeds to render itself.
struct Demo;

// Render returns AnyElement
// AnyElement is a wrapper for specific types of elements, which fromt the outside appear to be
// gerneric elements. Every specific element implements it's own type-specific Element-trait, so the
// methods layout, pre-paint and paint are defined. As AnyElement those appear to be generic and not
// element-specific and can be called anytime.
// This allows rendering in these three stages.
impl Render for Demo {
    fn render(&mut self, _window: &mut Window<'_>) -> AnyElement {
        quad(
            Rect::from_origin_and_size(Point::new(36.0, 72.0), Size::new(96.0, 56.0)),
            rgb(0.82, 0.29, 0.24),
        )
        .into_any_element()
    }
}
