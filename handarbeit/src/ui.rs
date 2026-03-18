use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use taffy::prelude::{
    AvailableSpace, Display, FlexDirection, NodeId, Position, Rect as TaffyRect, Size as TaffySize,
    Style, TaffyAuto, TaffyTree, auto, length,
};

use crate::geom::{Color, Point, Rect, Size, rgb};
use crate::gpu::DrawCmd;
use crate::text as text_system;

#[derive(Default)]
pub struct InputState {
    pub mouse_pos: Point,
    pub mouse_down: bool,
    pub mouse_pressed: bool,
    pub mouse_released: bool,
    pub press_pos: Option<Point>,
    pub release_pos: Option<Point>,
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
    // These are not in input state bc they are calculated using hitboxes, not from the OS
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

        if self
            .active
            .is_some_and(|id| !self.widgets.contains_key(&id))
        {
            self.active = None;
        }

        if self
            .hovered
            .is_some_and(|id| !self.widgets.contains_key(&id))
        {
            self.hovered = None;
        }
    }

    pub fn bump(&mut self, id: u64) {
        *self.ints.entry(id).or_insert(0) += 1;
    }

    pub fn get_int(&mut self, id: u64) -> i32 {
        *self.ints.entry(id).or_insert(0)
    }
}

#[derive(Default)]
#[allow(dead_code)]
struct WidgetState {
    id: u64,
    last_touched_frame: u64,
    #[allow(dead_code)]
    last_rect: Option<Rect>,
}

// Three stage rendering
// - request_layout: currently trivial, later calculates width and heigt
// - prepaint: will have to calculate hitboxes and interactove state
// - paint: actually returns the primitives that graphics can paint
// Any Element gors through these stages of layouting
pub trait Element: 'static {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn request_layout(&mut self, window: &mut Window<'_>) -> (NodeId, Self::RequestLayoutState);

    fn prepaint(
        &mut self,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_>,
    ) -> Self::PrepaintState;

    fn paint(
        &mut self,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    );
}

// This helps with quiet type conversion and hinding of the AnyElement() thing for outside
// libraries. It will allow for any element top be quietly converted into AnyElement behind the
// scenes without being too early or visible to the user (this lib can just into() it, and if
// the conversion is more complicated we can custom implement into_any_element()).
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

// This struct will be helpful: if the extend function is implemented we will get child() and
// children functions(), so that any element that is able to contain others can implement it.
pub trait ParentElement {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>);

    fn child(mut self, child: impl IntoElement) -> Self
    where
        Self: Sized,
    {
        self.extend(std::iter::once(child.into_any_element()));
        self
    }

    fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self
    where
        Self: Sized,
    {
        self.extend(children.into_iter().map(IntoElement::into_any_element));
        self
    }
}

pub struct Quad {
    rect: Rect,
    color: Color,
}

impl Quad {
    pub fn new(rect: Rect, color: Color) -> Self {
        Self { rect, color }
    }
}

impl Element for Quad {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, window: &mut Window<'_>) -> (NodeId, Self::RequestLayoutState) {
        let node_id = window
            .taffy
            .new_leaf(absolute_leaf_style(self.rect.min, self.rect.size()))
            .expect("create quad layout node");
        (node_id, ())
    }

    fn prepaint(
        &mut self,
        _bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        ()
    }

    fn paint(
        &mut self,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        window.draw_rect(bounds, self.color);
    }
}

// This is a wrapper around an element that can be drawn in three stages. It allows for calling
// layout and paint functions without knowing the concrete type that gets chain pushed to all these
// functions, which can be different for every other type of element (Quad, Div etc). it just can
// put in window and parent_origin, but dos know nothing about the concrete RequestLayoutState or
// PrepaintState, which is very pratical if you want to call these functions on every type.
pub struct AnyElement {
    inner: Box<dyn GenericElement>,
}

impl AnyElement {
    pub fn new<E: Element>(element: E) -> Self {
        Self {
            inner: Box::new(ElementBox::<E> {
                element,
                node_id: None,
                request_layout: None,
                prepaint: None,
                bounds: None,
            }),
        }
    }

    fn request_layout(&mut self, window: &mut Window<'_>) -> NodeId {
        self.inner.request_layout(window)
    }

    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_>) {
        self.inner.prepaint_from_parent(parent_origin, window);
    }

    pub fn prepaint_as_root(
        &mut self,
        origin: Point,
        available_size: Size,
        window: &mut Window<'_>,
    ) {
        let child = self.request_layout(window);
        let root = window
            .taffy
            .new_with_children(
                Style {
                    size: TaffySize {
                        width: length(available_size.width),
                        height: length(available_size.height),
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
                    width: AvailableSpace::Definite(available_size.width),
                    height: AvailableSpace::Definite(available_size.height),
                },
            )
            .expect("compute root layout");

        self.prepaint_from_parent(origin, window);
    }

    pub fn paint(&mut self, window: &mut Window<'_>) {
        self.inner.paint(window);
    }
}

pub struct Div {
    #[allow(dead_code)]
    id: Option<u64>,
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
        self.id = Some(hash_str(id_source));
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

    // Here we use laffy to add the children into our layout as children.
    fn request_layout(&mut self, window: &mut Window<'_>) -> (NodeId, Self::RequestLayoutState) {
        let children = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(window))
            .collect::<Vec<_>>();
        let node = window
            .taffy
            .new_with_children(self.style(), &children)
            .expect("create div node");
        (node, ())
    }

    fn prepaint(
        &mut self,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        for child in &mut self.children {
            child.prepaint_from_parent(bounds.min, window);
        }
    }

    fn paint(
        &mut self,
        bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        if let Some(color) = self.background {
            window.draw_rect(bounds, color);
        }

        for child in &mut self.children {
            child.paint(window);
        }
    }
}

pub fn div() -> Div {
    Div::new()
}

pub fn quad(rect: Rect, color: Color) -> Quad {
    Quad::new(rect, color)
}

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

    fn request_layout(&mut self, window: &mut Window<'_>) -> (NodeId, Self::RequestLayoutState) {
        let size = text_system::measure(&self.text, self.scale);
        let node = window
            .taffy
            .new_leaf(absolute_leaf_style(self.pos, size))
            .expect("create absolute text node");
        (node, TextRequestLayoutState)
    }

    fn prepaint(
        &mut self,
        _bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        ()
    }

    fn paint(
        &mut self,
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

    fn request_layout(&mut self, window: &mut Window<'_>) -> (NodeId, Self::RequestLayoutState) {
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
        _bounds: Rect,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        ()
    }

    fn paint(
        &mut self,
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

trait GenericElement {
    fn request_layout(&mut self, window: &mut Window<'_>) -> NodeId;
    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_>);
    fn paint(&mut self, window: &mut Window<'_>);
}

struct ElementBox<E: Element> {
    element: E,
    node_id: Option<NodeId>,
    request_layout: Option<E::RequestLayoutState>,
    prepaint: Option<E::PrepaintState>,
    bounds: Option<Rect>,
}

impl<E: Element> GenericElement for ElementBox<E> {
    fn request_layout(&mut self, window: &mut Window<'_>) -> NodeId {
        let (node_id, request_layout) = self.element.request_layout(window);
        self.node_id = Some(node_id);
        self.request_layout = Some(request_layout);
        self.prepaint = None;
        self.bounds = None;
        node_id
    }

    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_>) {
        let node_id = self.node_id.expect("request_layout must set a node id");
        let bounds = layout_rect(&window.taffy, node_id, parent_origin);
        let request_layout = self
            .request_layout
            .as_mut()
            .expect("request_layout must run before prepaint");
        let prepaint = self.element.prepaint(bounds, request_layout, window);
        self.bounds = Some(bounds);
        self.prepaint = Some(prepaint);
    }

    fn paint(&mut self, window: &mut Window<'_>) {
        let _node_id = self.node_id.expect("request_layout must set a node id");
        let bounds = self.bounds.expect("prepaint must run before paint");
        let request_layout = self
            .request_layout
            .as_mut()
            .expect("request_layout must run before paint");
        let prepaint = self
            .prepaint
            .as_mut()
            .expect("prepaint must run before paint");
        self.element.paint(bounds, request_layout, prepaint, window);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum UiAction {
    BumpInt(u64),
}

// 'static = cant store things in a struct that implements Render, which has it's own short lifetime
// and therefore determines the lifetime of the struct. It must be afully self-contained lifetime.
// It must contain only 'static data (like most primitive structs do).
pub trait Render: 'static {
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement;
}

pub struct Window<'a> {
    // We will use the memory later on. We will cache element state and dimensions of taffy
    // subtrees, also we will cache HarfBuzz shaping results here.
    memory: &'a mut UiMemory,
    input: &'a InputState,
    screen_size: Size,
    frame: u64,
    taffy: TaffyTree<()>,
    draw_list: Vec<DrawCmd>,
}

// Window stores transient state and gets recreated eevery frame.
// TODO: we have to reuse old state objects and not allocate this every frame.
impl<'a> Window<'a> {
    pub fn new(memory: &'a mut UiMemory, input: &'a InputState, screen_size: Size) -> Self {
        let frame = memory.frame;
        Self {
            memory,
            input,
            screen_size,
            frame,
            taffy: TaffyTree::new(),
            draw_list: Vec::new(),
        }
    }

    pub fn screen_size(&self) -> Size {
        self.screen_size
    }

    pub fn screen_rect(&self) -> Rect {
        Rect::from_origin_and_size(Point::origin(), self.screen_size)
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn counter(&mut self, id_source: &str) -> i32 {
        self.memory.get_int(root_id(id_source))
    }

    pub fn bump_counter_action(&self, id_source: &str) -> UiAction {
        UiAction::BumpInt(root_id(id_source))
    }

    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.draw_list.push(DrawCmd::Rect { rect, color });
    }

    pub fn draw_text(&mut self, pos: Point, text: impl Into<String>, scale: f32, color: Color) {
        self.draw_list.push(DrawCmd::Text {
            pos,
            text: text.into(),
            scale,
            color,
            clip_rect: None,
        });
    }

    pub fn finish(self) -> Vec<DrawCmd> {
        self.draw_list
    }
}

fn root_id(id_source: &str) -> u64 {
    hash_str(id_source)
}

fn layout_rect(taffy: &TaffyTree<()>, node_id: NodeId, parent_origin: Point) -> Rect {
    let layout = taffy.layout(node_id).expect("layout node");
    Rect::from_origin_and_size(
        Point::new(
            parent_origin.x + layout.location.x,
            parent_origin.y + layout.location.y,
        ),
        Size::new(layout.size.width, layout.size.height),
    )
}

fn absolute_leaf_style(pos: Point, size: Size) -> Style {
    Style {
        position: Position::Absolute,
        inset: inset(pos),
        size: TaffySize {
            width: length(size.width),
            height: length(size.height),
        },
        ..Default::default()
    }
}

fn inset(pos: Point) -> TaffyRect<taffy::style::LengthPercentageAuto> {
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

fn optional_size(size: Option<Size>) -> TaffySize<taffy::style::Dimension> {
    match size {
        Some(size) => TaffySize {
            width: length(size.width),
            height: length(size.height),
        },
        None => TaffySize::AUTO,
    }
}

fn hash_str(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
