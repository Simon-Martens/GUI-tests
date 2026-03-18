use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use taffy::prelude::{
    AvailableSpace, NodeId, Position, Rect as TaffyRect, Size as TaffySize, Style, TaffyAuto, auto,
    length,
};

use crate::geom::{Point, Rect, Size};

pub mod button;
pub mod div;
pub mod quad;
pub mod text;
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
    widgets: HashMap<u64, u64>,
}

impl UiMemory {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.hovered = None;
    }

    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.widgets
            .retain(|_, last_touched_frame| *last_touched_frame == frame);

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

    pub(super) fn touch_widget(&mut self, id: u64) {
        self.widgets.insert(id, self.frame);
    }
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
pub trait Element<Action: 'static>: 'static {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn id(&self) -> Option<LocalElementId> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<GlobalElementId>,
        window: &mut Window<'_, Action>,
    ) -> (NodeId, Self::RequestLayoutState);

    fn prepaint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_, Action>,
    ) -> Self::PrepaintState;

    fn paint(
        &mut self,
        id: Option<GlobalElementId>,
        bounds: Rect,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_, Action>,
    );
}

// This helps with quiet type conversion and hinding of the AnyElement() thing for outside
// libraries. It will allow for any element top be quietly converted into AnyElement behind the
// scenes without being too early or visible to the user (this lib can just into() it, and if
// the conversion is more complicated we can custom implement into_any_element()).
pub trait IntoElement<Action: 'static> {
    fn into_any_element(self) -> AnyElement<Action>;
}

impl<Action: 'static> IntoElement<Action> for AnyElement<Action> {
    fn into_any_element(self) -> AnyElement<Action> {
        self
    }
}

impl<Action: 'static> IntoElement<Action> for button::Button<Action> {
    fn into_any_element(self) -> AnyElement<Action> {
        AnyElement::new(self)
    }
}

impl<Action: 'static> IntoElement<Action> for div::Div<Action> {
    fn into_any_element(self) -> AnyElement<Action> {
        AnyElement::new(self)
    }
}

impl<Action: 'static> IntoElement<Action> for quad::Quad<Action> {
    fn into_any_element(self) -> AnyElement<Action> {
        AnyElement::new(self)
    }
}

impl<Action: 'static> IntoElement<Action> for text::Text<Action> {
    fn into_any_element(self) -> AnyElement<Action> {
        AnyElement::new(self)
    }
}

// This struct will be helpful: if the extend function is implemented we will get child() and
// children functions(), so that any element that is able to contain others can implement it.
pub trait ParentElement<Action: 'static> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement<Action>>);

    fn child(mut self, child: impl IntoElement<Action>) -> Self
    where
        Self: Sized,
    {
        self.extend(std::iter::once(child.into_any_element()));
        self
    }

    fn children(mut self, children: impl IntoIterator<Item = impl IntoElement<Action>>) -> Self
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
pub struct AnyElement<Action: 'static> {
    inner: Box<dyn GenericElement<Action>>,
}

impl<Action: 'static> AnyElement<Action> {
    pub fn new<E: Element<Action>>(element: E) -> Self {
        Self {
            inner: Box::new(ElementBox::<E, Action> {
                element,
                global_id: None,
                node_id: None,
                request_layout: None,
                prepaint: None,
                bounds: None,
                marker: PhantomData,
            }),
        }
    }

    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_, Action>,
    ) -> NodeId {
        self.inner.request_layout(parent_scope, window)
    }

    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_, Action>) {
        self.inner.prepaint_from_parent(parent_origin, window);
    }

    pub fn prepaint_as_root(
        &mut self,
        origin: Point,
        available_size: Size,
        window: &mut Window<'_, Action>,
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

    pub fn paint(&mut self, window: &mut Window<'_, Action>) {
        self.inner.paint(window);
    }
}

trait GenericElement<Action: 'static> {
    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_, Action>,
    ) -> NodeId;
    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_, Action>);
    fn paint(&mut self, window: &mut Window<'_, Action>);
}

struct ElementBox<E: Element<Action>, Action: 'static> {
    element: E,
    global_id: Option<GlobalElementId>,
    node_id: Option<NodeId>,
    request_layout: Option<E::RequestLayoutState>,
    prepaint: Option<E::PrepaintState>,
    bounds: Option<Rect>,
    marker: PhantomData<fn() -> Action>,
}

impl<E, Action> GenericElement<Action> for ElementBox<E, Action>
where
    E: Element<Action>,
    Action: 'static,
{
    fn request_layout(
        &mut self,
        parent_scope: Option<GlobalElementId>,
        window: &mut Window<'_, Action>,
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

    fn prepaint_from_parent(&mut self, parent_origin: Point, window: &mut Window<'_, Action>) {
        let node_id = self.node_id.expect("request_layout must set a node id");
        let bounds = window::layout_rect(&window.taffy, node_id, parent_origin);
        if let Some(id) = self.global_id {
            window.touch_widget(id);
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

    fn paint(&mut self, window: &mut Window<'_, Action>) {
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

// 'static = cant store things in a struct that implements Render, which has it's own short lifetime
// and therefore determines the lifetime of the struct. It must be afully self-contained lifetime.
// It must contain only 'static data (like most primitive structs do).
// NOTE: we have plastered Action everywhere to allow for message passing on certain hover / click
// etc. events to be handled by the state driver of the application. This is not very optimal since
// it confuse rendering the View with message parsing & state management, but rn I do not see a
// better way.
pub trait View: 'static {
    type Action: 'static;

    fn render(&mut self, window: &mut Window<'_, Self::Action>) -> AnyElement<Self::Action>;
}

pub trait Update<Action> {
    fn update(&mut self, action: Action);
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
