#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    app::run(
        Demo::default(),
        app::DebugOptions {
            time_frames: true,
        },
    );
}

#[derive(Default)]
struct Demo;

impl Render for Demo {
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement {
        let count = window.counter("button_count");
        let panel_pos = Point::new(
            window.screen_size().width * 0.5 - 110.0,
            window.screen_size().height * 0.5 - 55.0,
        );

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
                    .absolute(panel_pos)
                    .padding(18.0)
                    .gap(12.0)
                    .bg(rgb(0.14, 0.16, 0.20))
                    .child(label("WIDTH FROM CHILDREN"))
                    .child(
                        button("button", format!("BUTTON {count}"))
                            .on_click(window.bump_counter_action("button_count")),
                    ),
            )
            .into_any_element()
    }
}
