use taffy::prelude::NodeId;

use crate::geom::Color;
use crate::text as text_system;

use super::*;

pub struct AbsoluteText {
    pos: Point,
    text: String,
    scale: f32,
    color: Color,
}

impl AbsoluteText {
    fn new(pos: Point, text: String, scale: f32, color: Color) -> Self {
        Self {
            pos,
            text,
            scale,
            color,
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextRequestLayoutState;

impl Element for AbsoluteText {
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
            .new_leaf(absolute_leaf_style(self.pos, size))
            .expect("create absolute text node");
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

pub fn text(pos: Point, text: impl Into<String>, scale: f32, color: Color) -> AbsoluteText {
    AbsoluteText::new(pos, text.into(), scale, color)
}
