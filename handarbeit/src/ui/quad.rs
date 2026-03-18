use std::marker::PhantomData;

use crate::geom::Color;

use super::*;

pub struct Quad<Action: 'static> {
    rect: Rect,
    color: Color,
    block_mouse: bool,
    marker: PhantomData<fn() -> Action>,
}

impl<Action: 'static> Quad<Action> {
    pub fn new(rect: Rect, color: Color) -> Self {
        Self {
            rect,
            color,
            block_mouse: false,
            marker: PhantomData,
        }
    }

    #[allow(dead_code)]
    pub fn block_mouse(mut self) -> Self {
        self.block_mouse = true;
        self
    }
}

impl<Action: 'static> Element<Action> for Quad<Action> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(
        &mut self,
        _id: Option<GlobalElementId>,
        window: &mut Window<'_, Action>,
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
        window: &mut Window<'_, Action>,
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
        window: &mut Window<'_, Action>,
    ) {
        window.draw_rect(bounds, self.color);
    }
}

pub fn quad<Action: 'static>(rect: Rect, color: Color) -> Quad<Action> {
    Quad::new(rect, color)
}
