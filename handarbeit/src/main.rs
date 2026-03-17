mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::ui::{AnyElement, Render, Window};

fn main() {
    app::run(Demo::default());
}

#[derive(Default)]
struct Demo;

impl Render for Demo {
    fn render(&mut self, _window: &mut Window<'_>) -> AnyElement {
        AnyElement::default()
    }
}
