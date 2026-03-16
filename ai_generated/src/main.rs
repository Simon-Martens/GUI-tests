#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Rect, Vec2, rgb};
use crate::ui::{
    AnyElement, IntoElement, ParentElement, Render, Window, button, div, label, quad, text,
};

fn main() {
    app::run(Demo::default());
}

#[derive(Default)]
struct Demo;

impl Render for Demo {
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement {
        let count = window.counter("button_count");
        let panel_pos = Vec2::new(
            window.screen_size().x * 0.5 - 110.0,
            window.screen_size().y * 0.5 - 55.0,
        );

        div()
            .size(window.screen_size())
            .bg(rgb(0.08, 0.09, 0.11))
            .child(quad(
                Rect::from_min_size(Vec2::new(36.0, 72.0), Vec2::new(96.0, 56.0)),
                rgb(0.82, 0.29, 0.24),
            ))
            .child(text(
                Vec2::new(18.0, 18.0),
                format!("FRAMES {}", window.frame()),
                1.6,
                rgb(0.76, 0.80, 0.84),
            ))
            .child(text(
                Vec2::new(36.0, 144.0),
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
