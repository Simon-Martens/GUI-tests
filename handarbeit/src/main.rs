mod app;
mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Point, Rect, Size, rgb};
use crate::ui::absolute_text::text;
use crate::ui::button::button;
use crate::ui::div::div;
use crate::ui::label::label;
use crate::ui::quad::quad;
use crate::ui::{AnyElement, IntoElement, ParentElement, Render, Window};

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
        let screen = window.screen_size();
        let primary_count = window.counter("primary_count");
        let secondary_count = window.counter("secondary_count");
        let footer_count = window.counter("footer_count");
        let total_count = primary_count + secondary_count + footer_count;
        let panel_pos = Point::new(screen.width * 0.5 - 150.0, screen.height * 0.5 - 170.0);

        div()
            .size(screen)
            .bg(rgb(0.08, 0.09, 0.11))
            .child(quad(
                Rect::from_origin_and_size(Point::new(0.0, 0.0), Size::new(screen.width, 42.0)),
                rgb(0.11, 0.13, 0.17),
            ))
            .child(quad(
                Rect::from_origin_and_size(
                    Point::new(0.0, screen.height - 34.0),
                    Size::new(screen.width, 34.0),
                ),
                rgb(0.11, 0.13, 0.17),
            ))
            .child(quad(
                Rect::from_origin_and_size(Point::new(36.0, 72.0), Size::new(96.0, 56.0)),
                rgb(0.82, 0.29, 0.24),
            ))
            .child(text(
                Point::new(22.0, 12.0),
                "HANDARBEIT / TAFFY DEMO",
                1.4,
                rgb(0.93, 0.94, 0.96),
            ))
            .child(text(
                Point::new(36.0, 146.0),
                "ABSOLUTE PRIMITIVES STILL WORK",
                1.4,
                rgb(0.90, 0.92, 0.95),
            ))
            .child(text(
                Point::new(20.0, screen.height - 24.0),
                format!("TOTAL CLICKS {total_count}"),
                1.1,
                rgb(0.82, 0.85, 0.90),
            ))
            .child(
                div()
                    .id("panel")
                    .absolute(panel_pos)
                    .padding(18.0)
                    .gap(14.0)
                    .bg(rgb(0.14, 0.16, 0.20))
                    .child(
                        div()
                            .id("panel_header")
                            .size(Size::new(280.0, 42.0))
                            .padding(10.0)
                            .bg(rgb(0.19, 0.22, 0.27))
                            .child(label("CENTER PANEL / FIXED HEADER")),
                    )
                    .child(
                        div()
                            .id("primary_box")
                            .padding(12.0)
                            .gap(10.0)
                            .bg(rgb(0.17, 0.19, 0.24))
                            .child(label(format!("PRIMARY COUNTER {primary_count}")))
                            .child(
                                button("primary_button", format!("BUMP PRIMARY {primary_count}"))
                                    .on_click(window.bump_counter_action("primary_count")),
                            ),
                    )
                    .child(
                        div()
                            .id("nested_box")
                            .padding(12.0)
                            .gap(10.0)
                            .bg(rgb(0.16, 0.18, 0.23))
                            .child(label("NESTED DIV"))
                            .child(
                                div()
                                    .id("inner_box")
                                    .padding(10.0)
                                    .gap(8.0)
                                    .bg(rgb(0.20, 0.23, 0.29))
                                    .child(label(format!("SECONDARY {secondary_count}")))
                                    .child(
                                        button(
                                            "secondary_button",
                                            format!("BUMP SECONDARY {secondary_count}"),
                                        )
                                        .on_click(window.bump_counter_action("secondary_count")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("panel_footer")
                            .size(Size::new(280.0, 92.0))
                            .padding(10.0)
                            .gap(8.0)
                            .bg(rgb(0.19, 0.22, 0.27))
                            .child(label(format!("FOOTER COUNT {footer_count}")))
                            .child(
                                button("footer_button", format!("BUMP FOOTER {footer_count}"))
                                    .on_click(window.bump_counter_action("footer_count")),
                            ),
                    ),
            )
            .into_any_element()
    }
}
