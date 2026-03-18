use taffy::prelude::{NodeId, Size as TaffySize, Style, length};

use crate::geom::{Color, rgb};
use crate::text as text_system;
use crate::ui::absolute_text::TextRequestLayoutState;

use super::*;

pub struct Label {
    text: String,
    scale: f32,
    color: Color,
}

impl Label {
    fn new(text: String, scale: f32, color: Color) -> Self {
        Self { text, scale, color }
    }
}

impl Element for Label {
    type RequestLayoutState = TextRequestLayoutState;
    type PrepaintState = ();

    fn request_layout(
        &mut self,
        _id: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> (NodeId, Self::RequestLayoutState) {
        let size = text_system::measure(&self.text, self.scale);
        let node = window
            .taffy
            .new_leaf(Style {
                size: TaffySize {
                    width: length(size.width),
                    height: length(size.height),
                },
                ..Default::default()
            })
            .expect("create label node");
        (node, TextRequestLayoutState)
    }

    fn prepaint(
        &mut self,
        _id: Option<GlobalElementId>,
        _bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        ()
    }

    fn paint(
        &mut self,
        _id: Option<GlobalElementId>,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        window.draw_text(bounds.min, &self.text, self.scale, self.color);
    }
}

pub fn label(text: impl Into<String>) -> Label {
    Label::new(text.into(), 1.5, rgb(0.89, 0.91, 0.94))
}
