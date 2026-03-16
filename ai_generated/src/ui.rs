use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use taffy::prelude::{
    AvailableSpace, Display, FlexDirection, NodeId, Position, Rect as TaffyRect, Size as TaffySize,
    Style, TaffyTree, auto, length,
};

use crate::geom::{Color, Rect, Vec2, rgb};
use crate::gpu::DrawCmd;
use crate::text as text_system;

#[derive(Default)]
pub struct InputState {
    pub mouse_pos: Vec2,
    pub mouse_down: bool,
    pub mouse_pressed: bool,
    pub mouse_released: bool,
    pub press_pos: Option<Vec2>,
    pub release_pos: Option<Vec2>,
}

impl InputState {
    pub fn end_frame(&mut self) {
        self.mouse_pressed = false;
        self.mouse_released = false;
        self.press_pos = None;
        self.release_pos = None;
    }
}

#[derive(Default)]
pub struct UiMemory {
    frame: u64,
    pub hovered: Option<u64>,
    pub active: Option<u64>,
    ints: HashMap<u64, i32>,
    widgets: HashMap<u64, WidgetState>,
}

impl UiMemory {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.hovered = None;
    }

    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.widgets.retain(|id, state| {
            debug_assert_eq!(*id, state.id);
            state.last_touched_frame == frame
        });

        if self.active.is_some_and(|id| !self.widgets.contains_key(&id)) {
            self.active = None;
        }
        if self.hovered.is_some_and(|id| !self.widgets.contains_key(&id)) {
            self.hovered = None;
        }
    }

    pub fn bump(&mut self, id: u64) {
        *self.ints.entry(id).or_insert(0) += 1;
    }

    pub fn get_int(&mut self, id: u64) -> i32 {
        *self.ints.entry(id).or_insert(0)
    }

    fn touch_widget(&mut self, id: u64, rect: Rect) {
        let frame = self.frame;
        let state = self.widgets.entry(id).or_insert_with(|| WidgetState {
            id,
            ..Default::default()
        });
        state.last_touched_frame = frame;
        state.last_rect = Some(rect);
    }
}

#[derive(Default)]
struct WidgetState {
    id: u64,
    last_touched_frame: u64,
    last_rect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalElementId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlobalElementId(u64);

#[derive(Clone, Copy)]
struct Hitbox {
    id: Option<GlobalElementId>,
    rect: Rect,
    content_mask: Rect,
    behavior: HitboxBehavior,
    on_click: Option<UiAction>,
}

#[derive(Clone, Copy)]
enum HitboxBehavior {
    Clickable,
    BlockMouse,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameInteraction {
    pub hovered: Option<u64>,
    pub active: Option<u64>,
    pub clicked: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub enum UiAction {
    Clicked(u64),
    BumpInt(u64),
}

pub struct UiOutput {
    pub draw_list: Vec<DrawCmd>,
    pub actions: Vec<UiAction>,
    pub interaction: FrameInteraction,
}

pub trait Render: 'static {
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement;
}

pub struct Window<'a> {
    memory: &'a mut UiMemory,
    input: &'a InputState,
    screen_size: Vec2,
    frame: u64,
    taffy: TaffyTree<()>,
    hitboxes: Vec<Hitbox>,
    draw_list: Vec<DrawCmd>,
    actions: Vec<UiAction>,
    interaction: FrameInteraction,
    content_masks: Vec<Rect>,
}

impl<'a> Window<'a> {
    pub fn new(
        memory: &'a mut UiMemory,
        input: &'a InputState,
        screen_size: Vec2,
        frame: u64,
    ) -> Self {
        Self {
            memory,
            input,
            screen_size,
            frame,
            taffy: TaffyTree::new(),
            hitboxes: Vec::new(),
            draw_list: Vec::new(),
            actions: Vec::new(),
            interaction: FrameInteraction::default(),
            content_masks: vec![Rect::from_min_size(Vec2::ZERO, screen_size)],
        }
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn screen_size(&self) -> Vec2 {
        self.screen_size
    }

    pub fn screen_rect(&self) -> Rect {
        Rect::from_min_size(Vec2::ZERO, self.screen_size)
    }

    pub fn counter(&mut self, id_source: &str) -> i32 {
        self.memory.get_int(root_id(id_source))
    }

    pub fn bump_counter_action(&self, id_source: &str) -> UiAction {
        UiAction::BumpInt(root_id(id_source))
    }

    pub fn draw<R: Render>(&mut self, view: &mut R) -> UiOutput {
        self.taffy = TaffyTree::new();
        self.hitboxes.clear();
        self.draw_list.clear();
        self.actions.clear();
        self.interaction = FrameInteraction::default();
        self.content_masks.clear();
        self.content_masks.push(self.screen_rect());

        let mut root = view.render(self);
        root.prepaint_as_root(Vec2::ZERO, self.screen_size, self);
        self.interaction = self.resolve_interaction();
        root.paint(self);

        UiOutput {
            draw_list: std::mem::take(&mut self.draw_list),
            actions: std::mem::take(&mut self.actions),
            interaction: self.interaction,
        }
    }

    fn current_content_mask(&self) -> Rect {
        self.content_masks.last().copied().unwrap_or_else(|| self.screen_rect())
    }

    fn push_content_mask(&mut self, mask: Rect) {
        let next = self
            .current_content_mask()
            .intersect(mask)
            .unwrap_or_else(|| Rect::from_min_size(mask.min, Vec2::ZERO));
        self.content_masks.push(next);
    }

    fn pop_content_mask(&mut self) {
        if self.content_masks.len() > 1 {
            self.content_masks.pop();
        }
    }

    fn push_clickable_hitbox(
        &mut self,
        id: GlobalElementId,
        rect: Rect,
        action: Option<UiAction>,
    ) -> usize {
        let index = self.hitboxes.len();
        self.hitboxes.push(Hitbox {
            id: Some(id),
            rect,
            content_mask: self.current_content_mask(),
            behavior: HitboxBehavior::Clickable,
            on_click: action,
        });
        index
    }

    fn push_blocking_hitbox(&mut self, rect: Rect) {
        self.hitboxes.push(Hitbox {
            id: None,
            rect,
            content_mask: self.current_content_mask(),
            behavior: HitboxBehavior::BlockMouse,
            on_click: None,
        });
    }

    fn paint_quad(&mut self, rect: Rect, color: Color) {
        if let Some(rect) = rect.intersect(self.current_content_mask()) {
            self.draw_list.push(DrawCmd::Rect { rect, color });
        }
    }

    fn paint_text(&mut self, pos: Vec2, text: &str, scale: f32, color: Color) {
        let clip_rect = self.current_content_mask();
        if clip_rect.width() <= 0.0 || clip_rect.height() <= 0.0 {
            return;
        }
        self.draw_list.push(DrawCmd::Text {
            pos,
            text: text.to_string(),
            scale,
            color,
            clip_rect: Some(clip_rect),
        });
    }

    fn scoped_id(
        &self,
        parent_scope: Option<GlobalElementId>,
        local_id: LocalElementId,
    ) -> GlobalElementId {
        match parent_scope {
            Some(parent) => GlobalElementId(hash_u64(parent.0, local_id.0)),
            None => GlobalElementId(local_id.0),
        }
    }

    fn touch_widget(&mut self, id: GlobalElementId, rect: Rect) {
        self.memory.touch_widget(id.0, rect);
    }

    fn is_hovered(&self, id: GlobalElementId) -> bool {
        self.interaction.hovered == Some(id.0)
    }

    fn is_active(&self, id: GlobalElementId) -> bool {
        self.interaction.active == Some(id.0) && self.input.mouse_down
    }

    fn resolve_interaction(&mut self) -> FrameInteraction {
        let hovered_index = self.hit_test(self.input.mouse_pos);
        let hovered = hovered_index.and_then(|index| self.hitboxes[index].id).map(|id| id.0);
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
                if let Some(action) = hitbox.on_click {
                    self.actions.push(action);
                }
            }
        }

        self.memory.hovered = hovered;
        self.memory.active = active;

        FrameInteraction {
            hovered,
            active,
            clicked,
        }
    }

    fn hit_test(&self, point: Vec2) -> Option<usize> {
        for (index, hitbox) in self.hitboxes.iter().enumerate().rev() {
            let Some(visible_rect) = hitbox.rect.intersect(hitbox.content_mask) else {
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
}

trait Element: 'static {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn id(&self) -> Option<LocalElementId> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> (NodeId, Self::RequestLayoutState);

    fn prepaint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_>,
    ) -> Self::PrepaintState;

    fn paint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    );
}

pub trait IntoElement {
    fn into_any_element(self) -> AnyElement;
}

impl<T: Element> IntoElement for T {
    fn into_any_element(self) -> AnyElement {
        AnyElement::new(self)
    }
}

impl IntoElement for AnyElement {
    fn into_any_element(self) -> AnyElement {
        self
    }
}

pub trait ParentElement {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>);

    fn child(mut self, child: impl IntoElement) -> Self
    where
        Self: Sized,
    {
        self.extend(std::iter::once(child.into_any_element()));
        self
    }

    #[allow(dead_code)]
    fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self
    where
        Self: Sized,
    {
        self.extend(children.into_iter().map(IntoElement::into_any_element));
        self
    }
}

pub struct AnyElement {
    inner: Box<dyn ErasedElement>,
}

impl AnyElement {
    fn new<E: Element>(element: E) -> Self {
        Self {
            inner: Box::new(ElementBox::<E> {
                element,
                global_id: None,
                node_id: None,
                request_layout: None,
                prepaint: None,
                bounds: None,
            }),
        }
    }

    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> NodeId {
        self.inner.request_layout(parent_scope, window)
    }

    fn prepaint_from_parent(&mut self, parent_origin: Vec2, window: &mut Window<'_>) {
        self.inner.prepaint_from_parent(parent_origin, window);
    }

    pub fn prepaint_as_root(
        &mut self,
        origin: Vec2,
        available_size: Vec2,
        window: &mut Window<'_>,
    ) {
        let child = self.request_layout(None, window);
        let root = window
            .taffy
            .new_with_children(
                Style {
                    size: TaffySize {
                        width: length(available_size.x),
                        height: length(available_size.y),
                    },
                    ..Default::default()
                },
                &[child],
            )
            .expect("create root layout node");

        window
            .taffy
            .compute_layout(
                root,
                TaffySize {
                    width: AvailableSpace::Definite(available_size.x),
                    height: AvailableSpace::Definite(available_size.y),
                },
            )
            .expect("compute root layout");

        self.prepaint_from_parent(origin, window);
    }

    pub fn paint(&mut self, window: &mut Window<'_>) {
        self.inner.paint(window);
    }
}

trait ErasedElement {
    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> NodeId;
    fn prepaint_from_parent(&mut self, parent_origin: Vec2, window: &mut Window<'_>);
    fn paint(&mut self, window: &mut Window<'_>);
}

struct ElementBox<E: Element> {
    element: E,
    global_id: Option<GlobalElementId>,
    node_id: Option<NodeId>,
    request_layout: Option<E::RequestLayoutState>,
    prepaint: Option<E::PrepaintState>,
    bounds: Option<Rect>,
}

impl<E: Element> ErasedElement for ElementBox<E> {
    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> NodeId {
        let global_id = self
            .element
            .id()
            .map(|local_id| window.scoped_id(parent_scope, local_id));
        let (node_id, request_layout) = self.element.request_layout(global_id, window);
        self.global_id = global_id;
        self.node_id = Some(node_id);
        self.request_layout = Some(request_layout);
        self.prepaint = None;
        self.bounds = None;
        node_id
    }

    fn prepaint_from_parent(&mut self, parent_origin: Vec2, window: &mut Window<'_>) {
        let node_id = self.node_id.expect("missing node id before prepaint");
        let bounds = layout_rect(&window.taffy, node_id, parent_origin);
        self.bounds = Some(bounds);

        if let Some(id) = self.global_id {
            window.touch_widget(id, bounds);
        }

        let request_layout = self
            .request_layout
            .as_mut()
            .expect("missing request layout state before prepaint");
        let prepaint = self
            .element
            .prepaint(self.global_id, bounds, request_layout, window);
        self.prepaint = Some(prepaint);
    }

    fn paint(&mut self, window: &mut Window<'_>) {
        let bounds = self.bounds.expect("missing bounds before paint");
        let request_layout = self
            .request_layout
            .as_mut()
            .expect("missing request layout state before paint");
        let prepaint = self.prepaint.as_mut().expect("missing prepaint state before paint");
        self.element
            .paint(self.global_id, bounds, request_layout, prepaint, window);
    }
}

pub fn div() -> Div {
    Div::new()
}

pub fn quad(rect: Rect, color: Color) -> Quad {
    Quad::new(rect, color)
}

pub fn text(pos: Vec2, text: impl Into<String>, scale: f32, color: Color) -> AbsoluteText {
    AbsoluteText::new(pos, text.into(), scale, color)
}

pub fn label(text: impl Into<String>) -> Label {
    Label::new(text.into(), 1.5, rgb(0.89, 0.91, 0.94))
}

pub fn button(id_source: &str, label: impl Into<String>) -> Button {
    Button::new(LocalElementId(hash_str(id_source)), label.into(), 1.8)
}

pub struct Div {
    id: Option<LocalElementId>,
    position: Option<Vec2>,
    size: Option<Vec2>,
    padding: f32,
    gap: f32,
    background: Option<Color>,
    clip_children: bool,
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

    pub fn id(mut self, id_source: &str) -> Self {
        self.id = Some(LocalElementId(hash_str(id_source)));
        self
    }

    pub fn absolute(mut self, pos: Vec2) -> Self {
        self.position = Some(pos);
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
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
        if let Some(color) = self.background {
            window.paint_quad(bounds, color);
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

pub struct Quad {
    rect: Rect,
    color: Color,
    block_mouse: bool,
}

impl Quad {
    fn new(rect: Rect, color: Color) -> Self {
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
        let node = window
            .taffy
            .new_leaf(absolute_leaf_style(self.rect.min, self.rect.max - self.rect.min))
            .expect("create quad node");
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
        window.paint_quad(bounds, self.color);
    }
}

pub struct AbsoluteText {
    pos: Vec2,
    text: String,
    scale: f32,
    color: Color,
}

impl AbsoluteText {
    fn new(pos: Vec2, text: String, scale: f32, color: Color) -> Self {
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
        window.paint_text(bounds.min, &self.text, self.scale, self.color);
    }
}

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
                    width: length(size.x),
                    height: length(size.y),
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
        window.paint_text(bounds.min, &self.text, self.scale, self.color);
    }
}

pub struct Button {
    id: LocalElementId,
    label: String,
    scale: f32,
    padding: Vec2,
    on_click: Option<UiAction>,
}

pub struct ButtonRequestLayoutState {
    text_size: Vec2,
}

pub struct ButtonPrepaintState {
    hitbox_index: usize,
}

impl Button {
    fn new(id: LocalElementId, label: String, scale: f32) -> Self {
        Self {
            id,
            label,
            scale,
            padding: Vec2::new(14.0, 9.0),
            on_click: None,
        }
    }

    pub fn on_click(mut self, action: UiAction) -> Self {
        self.on_click = Some(action);
        self
    }
}

impl Element for Button {
    type RequestLayoutState = ButtonRequestLayoutState;
    type PrepaintState = ButtonPrepaintState;

    fn id(&self) -> Option<LocalElementId> {
        Some(self.id)
    }

    fn request_layout(
        &mut self,
        _id: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> (NodeId, Self::RequestLayoutState) {
        let text_size = text_system::measure(&self.label, self.scale);
        let size = text_size + self.padding * 2.0;
        let node = window
            .taffy
            .new_leaf(Style {
                size: TaffySize {
                    width: length(size.x),
                    height: length(size.y),
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
        let id = id.expect("button must have global id");
        let action = self.on_click.or(Some(UiAction::Clicked(id.0)));
        let hitbox_index = window.push_clickable_hitbox(id, bounds, action);
        ButtonPrepaintState { hitbox_index }
    }

    fn paint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        let id = id.expect("button must have global id");
        let _ = prepaint.hitbox_index;
        let background = if window.is_active(id) {
            rgb(0.93, 0.74, 0.45)
        } else if window.is_hovered(id) {
            rgb(0.85, 0.81, 0.70)
        } else {
            rgb(0.78, 0.78, 0.75)
        };

        window.paint_quad(bounds, background);

        let text_pos = Vec2::new(
            bounds.min.x + (bounds.width() - request_layout.text_size.x) * 0.5,
            bounds.min.y + (bounds.height() - request_layout.text_size.y) * 0.5,
        );
        window.paint_text(text_pos, &self.label, self.scale, rgb(0.08, 0.09, 0.11));
    }
}

fn layout_rect(taffy: &TaffyTree<()>, node_id: NodeId, parent_origin: Vec2) -> Rect {
    let layout = taffy.layout(node_id).expect("layout node");
    Rect::from_min_size(
        parent_origin + Vec2::new(layout.location.x, layout.location.y),
        Vec2::new(layout.size.width, layout.size.height),
    )
}

fn absolute_leaf_style(pos: Vec2, size: Vec2) -> Style {
    Style {
        position: Position::Absolute,
        inset: inset(pos),
        size: TaffySize {
            width: length(size.x),
            height: length(size.y),
        },
        ..Default::default()
    }
}

fn inset(pos: Vec2) -> TaffyRect<taffy::style::LengthPercentageAuto> {
    TaffyRect {
        left: length(pos.x),
        right: auto(),
        top: length(pos.y),
        bottom: auto(),
    }
}

fn all_sides(value: f32) -> TaffyRect<taffy::style::LengthPercentage> {
    TaffyRect {
        left: length(value),
        right: length(value),
        top: length(value),
        bottom: length(value),
    }
}

fn optional_size(size: Option<Vec2>) -> TaffySize<taffy::style::Dimension> {
    match size {
        Some(size) => TaffySize {
            width: length(size.x),
            height: length(size.y),
        },
        None => TaffySize {
            width: auto(),
            height: auto(),
        },
    }
}

fn root_id(id_source: &str) -> u64 {
    hash_str(id_source)
}

fn hash_str(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn hash_u64(a: u64, b: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut hasher);
    b.hash(&mut hasher);
    hasher.finish()
}
