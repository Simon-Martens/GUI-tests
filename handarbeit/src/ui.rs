use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::geom::{Color, Point, Rect, Size};
use crate::gpu::DrawCmd;

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

pub trait Element: 'static {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn request_layout(&mut self, window: &mut Window<'_>) -> Self::RequestLayoutState;

    fn prepaint(
        &mut self,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window<'_>,
    ) -> Self::PrepaintState;

    fn paint(
        &mut self,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    );
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

// Three stage rendering
// - request_layout: currently trivial, later calculates width and heigt
// - prepaint: will have to calculate hitboxes and interactove state
// - paint: actually returns the primitives that graphics can paint
// Any Element gors through these stages of layouting
impl Element for Quad {
    type RequestLayoutState = Rect;
    type PrepaintState = Rect;

    fn request_layout(&mut self, _window: &mut Window<'_>) -> Self::RequestLayoutState {
        self.rect
    }

    fn prepaint(
        &mut self,
        request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window<'_>,
    ) -> Self::PrepaintState {
        *request_layout
    }

    fn paint(
        &mut self,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window<'_>,
    ) {
        window.draw_rect(*prepaint, self.color);
    }
}

pub struct AnyElement {
    quad: Quad,
    request_layout: Option<<Quad as Element>::RequestLayoutState>,
    prepaint: Option<<Quad as Element>::PrepaintState>,
}

impl AnyElement {
    pub fn new(quad: Quad) -> Self {
        Self {
            quad,
            request_layout: None,
            prepaint: None,
        }
    }

    pub fn request_layout(&mut self, window: &mut Window<'_>) {
        self.request_layout = Some(self.quad.request_layout(window));
        self.prepaint = None;
    }

    pub fn prepaint(&mut self, window: &mut Window<'_>) {
        let request_layout = self
            .request_layout
            .as_mut()
            .expect("request_layout must run before prepaint");
        self.prepaint = Some(self.quad.prepaint(request_layout, window));
    }

    pub fn paint(&mut self, window: &mut Window<'_>) {
        let request_layout = self
            .request_layout
            .as_mut()
            .expect("request_layout must run before paint");
        let prepaint = self
            .prepaint
            .as_mut()
            .expect("prepaint must run before paint");
        self.quad.paint(request_layout, prepaint, window);
    }
}

pub fn quad(rect: Rect, color: Color) -> AnyElement {
    AnyElement::new(Quad::new(rect, color))
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
    memory: &'a mut UiMemory,
    #[allow(dead_code)]
    input: &'a InputState,
    screen_size: Size,
    frame: u64,
    draw_list: Vec<DrawCmd>,
}

impl<'a> Window<'a> {
    pub fn new(memory: &'a mut UiMemory, input: &'a InputState, screen_size: Size) -> Self {
        let frame = memory.frame;
        Self {
            memory,
            input,
            screen_size,
            frame,
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

fn hash_str(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
