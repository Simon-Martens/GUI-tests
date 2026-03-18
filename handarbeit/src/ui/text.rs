use std::marker::PhantomData;

use taffy::prelude::{NodeId, Size as TaffySize, Style, length};

use crate::geom::Color;
use crate::text as text_system;

use super::*;

pub struct Text<Action: 'static> {
    position: Option<Point>,
    text: String,
    scale: f32,
    color: Color,
    marker: PhantomData<fn() -> Action>,
}

impl<Action: 'static> Text<Action> {
    fn new(text: String, scale: f32, color: Color) -> Self {
        Self {
            position: None,
            text,
            scale,
            color,
            marker: PhantomData,
        }
    }

    pub fn absolute(mut self, pos: Point) -> Self {
        self.position = Some(pos);
        self
    }
}

impl<Action: 'static> Element<Action> for Text<Action> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(
        &mut self,
        _id: Option<GlobalElementId>,
        window: &mut Window<'_, Action>,
    ) -> (NodeId, Self::RequestLayoutState) {
        let size = text_system::measure(&self.text, self.scale);
        let style = match self.position {
            Some(pos) => absolute_leaf_style(pos, size),
            None => Style {
                size: TaffySize {
                    width: length(size.width),
                    height: length(size.height),
                },
                ..Default::default()
            },
        };
        let node = window.taffy.new_leaf(style).expect("create text node");
        (node, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<GlobalElementId>,
        _bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window<'_, Action>,
    ) -> Self::PrepaintState {
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
        window.draw_text(bounds.min, &self.text, self.scale, self.color);
    }
}

pub fn text<Action: 'static>(text: impl Into<String>, scale: f32, color: Color) -> Text<Action> {
    Text::new(text.into(), scale, color)
}
