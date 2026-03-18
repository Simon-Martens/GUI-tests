use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use taffy::prelude::{
    AvailableSpace, NodeId, Position, Rect as TaffyRect, Size as TaffySize, Style, TaffyAuto, auto,
    length,
};

use crate::geom::{Point, Rect, Size};

pub mod absolute_text;
pub mod button;
pub mod div;
pub mod label;
pub mod quad;
mod window;

pub use window::Window;

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
    pub(super) frame: u64,
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

    pub(super) fn touch_widget(&mut self, id: u64, rect: Rect) {
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
#[allow(dead_code)]
struct WidgetState {
    id: u64,
    last_touched_frame: u64,
    #[allow(dead_code)]
    // TODO: We don't use this for anything, maybe it might become a state buffer sometime?
    last_rect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalElementId(pub(crate) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GlobalElementId(pub(crate) u64);

// Three stage rendering
// - request_layout: currently trivial, later calculates width and heigt
// - prepaint: will have to calculate hitboxes and interactove state
// - paint: actually returns the primitives that graphics can paint
// Any Element gors through these stages of layouting
pub trait Element: 'static {
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

    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_>) {
        self.inner.prepaint_from_parent(parent_origin, window);
    }

    pub fn prepaint_as_root(
        &mut self,
        origin: Point,
        available_size: Size,
        window: &mut Window<'_>,
    ) {
        let child = self.request_layout(None, window);
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

trait GenericElement {
    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_>,
    ) -> NodeId;
    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_>);
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

impl<E: Element> GenericElement for ElementBox<E> {
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

    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_>) {
        let node_id = self.node_id.expect("request_layout must set a node id");
        let bounds = window::layout_rect(&window.taffy, node_id, parent_origin);
        if let Some(id) = self.global_id {
            window.touch_widget(id, bounds);
        }
        let request_layout = self
            .request_layout
            .as_mut()
            .expect("request_layout must run before prepaint");
        let prepaint = self
            .element
            .prepaint(self.global_id, bounds, request_layout, window);
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
        self.element
            .paint(self.global_id, bounds, request_layout, prepaint, window);
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

pub(super) fn root_id(id_source: &str) -> u64 {
    hash_str(id_source)
}

pub(super) fn absolute_leaf_style(pos: Point, size: Size) -> Style {
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

pub(super) fn inset(pos: Point) -> TaffyRect<taffy::style::LengthPercentageAuto> {
    TaffyRect {
        left: length(pos.x),
        right: auto(),
        top: length(pos.y),
        bottom: auto(),
    }
}

pub(super) fn all_sides(value: f32) -> TaffyRect<taffy::style::LengthPercentage> {
    TaffyRect {
        left: length(value),
        right: length(value),
        top: length(value),
        bottom: length(value),
    }
}

pub(super) fn optional_size(size: Option<Size>) -> TaffySize<taffy::style::Dimension> {
    match size {
        Some(size) => TaffySize {
            width: length(size.width),
            height: length(size.height),
        },
        None => TaffySize::AUTO,
    }
}

pub(super) fn hash_str(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn hash_u64(a: u64, b: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut hasher);
    b.hash(&mut hasher);
    hasher.finish()
}
