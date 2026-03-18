use crate::geom::Color;

use super::*;

pub struct Quad {
    rect: Rect,
    color: Color,
    block_mouse: bool,
}

impl Quad {
    pub fn new(rect: Rect, color: Color) -> Self {
        Self {
            rect,
            color,
            block_mouse: false,
        }
    }

    #[allow(dead_code)]
    pub fn block_mouse(mut self) -> Self {
        self.block_mouse = true;
        self
    }
}

impl Element for Quad {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(
        &mut self,
        _id: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> (NodeId, Self::RequestLayoutState) {
        let node_id = window
            .taffy
            .new_leaf(absolute_leaf_style(self.rect.min, self.rect.size()))
            .expect("create quad layout node");
        (node_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<GlobalElementId>,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        if self.block_mouse {
            window.push_blocking_hitbox(bounds);
        }
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
        window.draw_rect(bounds, self.color);
    }
}

pub fn quad(rect: Rect, color: Color) -> Quad {
    Quad::new(rect, color)
}
