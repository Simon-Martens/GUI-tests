use std::collections::HashMap;
use std::marker::PhantomData;

use crate::geom::{Rect, Size};

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
struct WidgetState {
    id: u64,
    last_touched_frame: u64,
    #[allow(dead_code)]
    last_rect: Option<Rect>,
}

#[derive(Default)]
pub struct AnyElement;

pub trait Render: 'static {
    fn render(&mut self, window: &mut Window<'_>) -> AnyElement;
}

pub struct Window<'a> {
    #[allow(dead_code)]
    screen_size: Size,
    #[allow(dead_code)]
    frame: u64,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a> Window<'a> {
    pub fn new(screen_size: Size, frame: u64) -> Self {
        Self {
            screen_size,
            frame,
            _marker: PhantomData,
        }
    }
}
