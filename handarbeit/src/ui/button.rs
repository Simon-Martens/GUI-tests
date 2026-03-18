use taffy::prelude::{NodeId, Size as TaffySize, Style, length};

use crate::geom::rgb;
use crate::text as text_system;

use super::*;

pub struct Button {
    #[allow(dead_code)]
    id: LocalElementId,
    label: String,
    scale: f32,
    padding: Size,
    #[allow(dead_code)]
    on_click: Option<UiAction>,
}

pub struct ButtonRequestLayoutState {
    text_size: Size,
}

impl Button {
    fn new(id: LocalElementId, label: String, scale: f32) -> Self {
        Self {
            id,
            label,
            scale,
            padding: Size::new(14.0, 9.0),
            on_click: None,
        }
    }

    #[allow(dead_code)]
    pub fn on_click(mut self, action: UiAction) -> Self {
        self.on_click = Some(action);
        self
    }
}

impl Element for Button {
    type RequestLayoutState = ButtonRequestLayoutState;
    type PrepaintState = ();

    fn id(&self) -> Option<LocalElementId> {
        Some(self.id)
    }

    fn request_layout(
        &mut self,
        _id: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> (NodeId, Self::RequestLayoutState) {
        let text_size = text_system::measure(&self.label, self.scale);
        let size = Size::new(
            text_size.width + self.padding.width * 2.0,
            text_size.height + self.padding.height * 2.0,
        );
        let node = window
            .taffy
            .new_leaf(Style {
                size: TaffySize {
                    width: length(size.width),
                    height: length(size.height),
                },
                ..Default::default()
            })
            .expect("create button node");
        (node, ButtonRequestLayoutState { text_size })
    }

    fn prepaint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        if let Some(id) = id {
            let _ = window.push_clickable_hitbox(id, bounds, self.on_click);
        }
        ()
    }

    fn paint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        let id = id.expect("button must have global id");
        let background = if window.is_active(id) {
            rgb(0.93, 0.74, 0.45)
        } else if window.is_hovered(id) {
            rgb(0.36, 0.41, 0.49)
        } else {
            rgb(0.26, 0.30, 0.37)
        };

        window.draw_rect(bounds, background);
        let text_pos = Point::new(
            bounds.min.x + self.padding.width,
            bounds.min.y + (bounds.height() - request_layout.text_size.height) * 0.5,
        );
        window.draw_text(text_pos, &self.label, self.scale, rgb(0.95, 0.96, 0.98));
    }
}

pub fn button(id_source: &str, label: impl Into<String>) -> Button {
    Button::new(LocalElementId(hash_str(id_source)), label.into(), 1.8)
}
