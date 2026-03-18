mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Point, Rect, Size, rgb};
use crate::ui::{
    AnyElement, IntoElement, ParentElement, Render, Window, button, div, label, quad, text,
};

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
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement {
        div()
            .size(window.screen_size())
            .bg(rgb(0.08, 0.09, 0.11))
            .child(quad(
                Rect::from_origin_and_size(Point::new(36.0, 72.0), Size::new(96.0, 56.0)),
                rgb(0.82, 0.29, 0.24),
            ))
            .child(text(
                Point::new(36.0, 144.0),
                "DRAWN FROM main.rs",
                1.4,
                rgb(0.90, 0.92, 0.95),
            ))
            .child(
                div()
                    .id("panel")
                    .absolute(Point::new(260.0, 120.0))
                    .padding(18.0)
                    .gap(12.0)
                    .bg(rgb(0.14, 0.16, 0.20))
                    .child(label("WIDTH FROM CHILDREN"))
                    .child(button("button", "BUTTON")),
            )
            .into_any_element()
    }
}
