mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Point, Rect, Size, rgb};
use crate::ui::{AnyElement, Render, Window};

fn main() {
    app::run(Demo);
}

// INFO: Demo is our state struct. It is retained across frames and should own all the state it
// neeeds to render itself.
struct Demo;

impl Render for Demo {
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement {
        let screen_size = window.screen_size();
        let frame = window.frame();
        let count = window.counter("demo_counter");
        let accent_width = (screen_size.width - 72.0).clamp(0.0, 96.0);

        window.draw_rect(window.screen_rect(), rgb(0.08, 0.09, 0.11));
        window.draw_rect(
            Rect::from_origin_and_size(Point::new(36.0, 72.0), Size::new(accent_width, 56.0)),
            rgb(0.82, 0.29, 0.24),
        );
        window.draw_text(
            Point::new(36.0, 144.0),
            format!("DRAWN FROM Render::render()  FRAME {frame}  COUNT {count}"),
            1.4,
            rgb(0.90, 0.92, 0.95),
        );

        AnyElement::new()
    }
}
