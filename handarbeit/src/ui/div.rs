use taffy::prelude::{Display, FlexDirection, NodeId, Position, Size as TaffySize, Style, length};

use crate::geom::Color;

use super::*;

pub struct Div {
    #[allow(dead_code)]
    id: Option<LocalElementId>,
    position: Option<Point>,
    size: Option<Size>,
    padding: f32,
    gap: f32,
    background: Option<Color>,
    #[allow(dead_code)]
    clip_children: bool,
    #[allow(dead_code)]
    block_mouse: bool,
    children: Vec<AnyElement>,
}

impl Div {
    fn new() -> Self {
        Self {
            id: None,
            position: None,
            size: None,
            padding: 0.0,
            gap: 0.0,
            background: None,
            clip_children: true,
            block_mouse: false,
            children: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn id(mut self, id_source: &str) -> Self {
        self.id = Some(LocalElementId(hash_str(id_source)));
        self
    }

    pub fn absolute(mut self, pos: Point) -> Self {
        self.position = Some(pos);
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    #[allow(dead_code)]
    pub fn clip(mut self, clip_children: bool) -> Self {
        self.clip_children = clip_children;
        self
    }

    #[allow(dead_code)]
    pub fn block_mouse(mut self) -> Self {
        self.block_mouse = true;
        self
    }

    fn style(&self) -> Style {
        let mut style = Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            padding: all_sides(self.padding),
            gap: TaffySize {
                width: length(self.gap),
                height: length(self.gap),
            },
            size: optional_size(self.size),
            ..Default::default()
        };

        if let Some(pos) = self.position {
            style.position = Position::Absolute;
            style.inset = inset(pos);
        }

        style
    }
}

impl ParentElement for Div {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Element for Div {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<LocalElementId> {
        self.id
    }

    // Here we use laffy to add the children into our layout as children.
    fn request_layout(
        &mut self,
        id: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> (NodeId, Self::RequestLayoutState) {
        let children = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(id, window))
            .collect::<Vec<_>>();
        let node = window
            .taffy
            .new_with_children(self.style(), &children)
            .expect("create div node");
        (node, ())
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
        if self.clip_children {
            window.push_content_mask(bounds);
        }
        for child in &mut self.children {
            child.prepaint_from_parent(bounds.min, window);
        }
        if self.clip_children {
            window.pop_content_mask();
        }
    }

    fn paint(
        &mut self,
        _id: Option<GlobalElementId>,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        if let Some(color) = self.background {
            window.draw_rect(bounds, color);
        }

        if self.clip_children {
            window.push_content_mask(bounds);
        }
        for child in &mut self.children {
            child.paint(window);
        }
        if self.clip_children {
            window.pop_content_mask();
        }
    }
}

pub fn div() -> Div {
    Div::new()
}
