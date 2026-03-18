use taffy::prelude::{NodeId, TaffyTree};

use crate::geom::Color;
use crate::gpu::DrawCmd;

use super::*;

#[derive(Clone, Copy)]
struct Hitbox {
    id: Option<GlobalElementId>,
    rect: Rect,
    content_mask: Rect,
    behavior: HitboxBehavior,
    on_click: Option<usize>,
}

#[derive(Clone, Copy)]
enum HitboxBehavior {
    Clickable,
    BlockMouse,
}

// Here we save IDs of clicked or hovered items in a frame
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameInteraction {
    pub hovered: Option<u64>,
    pub active: Option<u64>,
}

pub struct UiOutput<Action> {
    pub draw_list: Vec<DrawCmd>,
    pub actions: Vec<Action>,
}

pub struct Window<'a, Action: 'static> {
    // We will use the memory later on. We will cache element state and dimensions of taffy
    // subtrees, also we will cache HarfBuzz shaping results here.
    pub(super) memory: &'a mut UiMemory,
    pub(super) input: &'a InputState,
    pub(super) screen_size: Size,
    pub(super) taffy: TaffyTree<()>,
    // Hitboxes: can be clocking (no mouse events registered underneath) and/or clickable (clickable
    // elements like buttons or links). We save clicked or hovered ids.
    // Hitboxes: can be blocking and/or clickable: blocking hitboxes prevent registering
    // click items underneath. We collect them seperate to make hit testing very fast. Hit
    // testing is done below in window with resolve hit and hit test functions.
    hitboxes: Vec<Hitbox>,
    // Here we store ids of hovered or clicked upon items.
    interaction: FrameInteraction,
    // Masks: made for clipping content.
    // TODO: GPU clipping. Right now we do it on the CPU.
    content_masks: Vec<Rect>,
    // This will be part of our results pipeleine for rendering. All items can add and queue actions and
    // here to be executed (or not) or drawn (or not). We do not execute from the items or in the
    // render path directly, istead just queue actions.
    click_actions: Vec<Option<Action>>,
    actions: Vec<Action>,
    draw_list: Vec<DrawCmd>,
}

// Window stores transient state and gets recreated eevery frame.
// TODO: we have to reuse old state objects and not allocate this every frame.
impl<'a, Action: 'static> Window<'a, Action> {
    pub fn new(memory: &'a mut UiMemory, input: &'a InputState, screen_size: Size) -> Self {
        Self {
            memory,
            input,
            screen_size,
            taffy: TaffyTree::new(),
            hitboxes: Vec::new(),
            interaction: FrameInteraction::default(),
            content_masks: vec![Rect::from_origin_and_size(Point::origin(), screen_size)],
            click_actions: Vec::new(),
            actions: Vec::new(),
            draw_list: Vec::new(),
        }
    }

    pub fn screen_size(&self) -> Size {
        self.screen_size
    }

    pub fn screen_rect(&self) -> Rect {
        Rect::from_origin_and_size(Point::origin(), self.screen_size)
    }

    pub fn draw<R: View<Action = Action>>(&mut self, view: &mut R) -> UiOutput<Action> {
        self.taffy = TaffyTree::new();
        self.hitboxes.clear();
        self.interaction = FrameInteraction::default();
        self.content_masks.clear();
        self.content_masks.push(self.screen_rect());
        self.click_actions.clear();
        self.actions.clear();
        self.draw_list.clear();

        let mut root = view.render(self);
        root.prepaint_as_root(Point::origin(), self.screen_size, self);
        self.interaction = self.resolve_interaction();
        root.paint(self);

        UiOutput {
            draw_list: std::mem::take(&mut self.draw_list),
            actions: std::mem::take(&mut self.actions),
        }
    }

    pub(super) fn scoped_id(
        &self,
        parent_scope: Option<GlobalElementId>,
        local_id: LocalElementId,
    ) -> GlobalElementId {
        match parent_scope {
            Some(parent) => GlobalElementId(hash_u64(parent.0, local_id.0)),
            None => GlobalElementId(local_id.0),
        }
    }

    pub(super) fn touch_widget(&mut self, id: GlobalElementId) {
        self.memory.touch_widget(id.0);
    }

    pub(super) fn current_content_mask(&self) -> Rect {
        self.content_masks
            .last()
            .copied()
            .unwrap_or_else(|| self.screen_rect())
    }

    pub(super) fn push_content_mask(&mut self, mask: Rect) {
        let next = self
            .current_content_mask()
            .intersection(&mask)
            .unwrap_or_else(|| Rect::from_origin_and_size(mask.min, Size::new(0.0, 0.0)));
        self.content_masks.push(next);
    }

    pub(super) fn pop_content_mask(&mut self) {
        if self.content_masks.len() > 1 {
            self.content_masks.pop();
        }
    }

    pub(super) fn push_clickable_hitbox(
        &mut self,
        id: GlobalElementId,
        rect: Rect,
        action: Option<Action>,
    ) {
        let on_click = action.map(|action| {
            self.click_actions.push(Some(action));
            self.click_actions.len() - 1
        });
        self.hitboxes.push(Hitbox {
            id: Some(id),
            rect,
            content_mask: self.current_content_mask(),
            behavior: HitboxBehavior::Clickable,
            on_click,
        });
    }

    pub(super) fn push_blocking_hitbox(&mut self, rect: Rect) {
        self.hitboxes.push(Hitbox {
            id: None,
            rect,
            content_mask: self.current_content_mask(),
            behavior: HitboxBehavior::BlockMouse,
            on_click: None,
        });
    }

    pub(super) fn is_hovered(&self, id: GlobalElementId) -> bool {
        self.interaction.hovered == Some(id.0)
    }

    pub(super) fn is_active(&self, id: GlobalElementId) -> bool {
        self.interaction.active == Some(id.0) && self.input.mouse_down
    }

    fn resolve_interaction(&mut self) -> FrameInteraction {
        let hovered_index = self.hit_test(self.input.mouse_pos);
        let hovered = hovered_index
            .and_then(|index| self.hitboxes[index].id)
            .map(|id| id.0);
        let previous_active = self.memory.active;

        let active = if self.input.mouse_pressed {
            hovered
        } else if self.input.mouse_released {
            None
        } else if self.input.mouse_down {
            previous_active
        } else {
            None
        };

        let clicked = if self.input.mouse_released && hovered == previous_active {
            hovered
        } else {
            None
        };

        if let Some(index) = hovered_index {
            let hitbox = self.hitboxes[index];
            if hitbox.id.map(|id| id.0) == clicked {
                if let Some(action_index) = hitbox.on_click {
                    if let Some(action) = self.click_actions[action_index].take() {
                        self.actions.push(action);
                    }
                }
            }
        }

        self.memory.hovered = hovered;
        self.memory.active = active;

        FrameInteraction { hovered, active }
    }

    fn hit_test(&self, point: Point) -> Option<usize> {
        for (index, hitbox) in self.hitboxes.iter().enumerate().rev() {
            let Some(visible_rect) = hitbox.rect.intersection(&hitbox.content_mask) else {
                continue;
            };
            if visible_rect.contains(point) {
                return match hitbox.behavior {
                    HitboxBehavior::Clickable | HitboxBehavior::BlockMouse => Some(index),
                };
            }
        }
        None
    }

    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        if let Some(rect) = rect.intersection(&self.current_content_mask()) {
            self.draw_list.push(DrawCmd::Rect { rect, color });
        }
    }

    pub fn draw_text(&mut self, pos: Point, text: impl Into<String>, scale: f32, color: Color) {
        let clip_rect = self.current_content_mask();
        if clip_rect.width() <= 0.0 || clip_rect.height() <= 0.0 {
            return;
        }
        self.draw_list.push(DrawCmd::Text {
            pos,
            text: text.into(),
            scale,
            color,
            clip_rect: Some(clip_rect),
        });
    }
}

pub(super) fn layout_rect(taffy: &TaffyTree<()>, node_id: NodeId, parent_origin: Point) -> Rect {
    let layout = taffy.layout(node_id).expect("layout node");
    Rect::from_origin_and_size(
        Point::new(
            parent_origin.x + layout.location.x,
            parent_origin.y + layout.location.y,
        ),
        Size::new(layout.size.width, layout.size.height),
    )
}
