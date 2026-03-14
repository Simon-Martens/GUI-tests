use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::geom::{Color, Rect, Vec2, rgb};
use crate::gpu::DrawCmd;

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
    pub hot: Option<u64>,
    pub active: Option<u64>,
    ints: HashMap<u64, i32>,
}

impl UiMemory {
    pub fn begin_frame(&mut self) {
        self.hot = None;
    }

    pub fn bump(&mut self, id: u64) {
        *self.ints.entry(id).or_insert(0) += 1;
    }

    pub fn get_int(&mut self, id: u64) -> i32 {
        *self.ints.entry(id).or_insert(0)
    }
}

pub struct Ui<'a> {
    memory: &'a mut UiMemory,
    input: &'a InputState,
    pub screen_size: Vec2,
    draw: Vec<DrawCmd>,
    parents: Vec<Parent>,
}

impl<'a> Ui<'a> {
    pub fn new(memory: &'a mut UiMemory, input: &'a InputState, screen_size: Vec2) -> Self {
        Self {
            memory,
            input,
            screen_size,
            draw: Vec::new(),
            parents: Vec::new(),
        }
    }

    pub fn finish(self) -> Vec<DrawCmd> {
        self.draw
    }

    pub fn fill(&mut self, rect: Rect, color: Color) {
        self.draw.push(DrawCmd::Rect { rect, color });
    }

    pub fn text(&mut self, pos: Vec2, text: impl Into<String>, scale: f32, color: Color) {
        self.draw.push(DrawCmd::Text {
            pos,
            text: text.into(),
            scale,
            color,
        });
    }

    pub fn begin_root_panel(
        &mut self,
        id_source: &str,
        rect: Rect,
        padding: f32,
        spacing: f32,
        color: Color,
    ) {
        self.begin_panel_with_rect(self.root_id(id_source), rect, padding, spacing, color);
    }

    pub fn begin_child_panel(
        &mut self,
        id_source: &str,
        height: f32,
        padding: f32,
        spacing: f32,
        color: Color,
    ) {
        let rect = self.next_rect(height);
        let id = self.scoped_id(id_source);
        self.begin_panel_with_rect(id, rect, padding, spacing, color);
    }

    pub fn end_panel(&mut self) {
        self.parents.pop();
    }

    pub fn label(&mut self, text: &str) {
        let rect = self.next_rect(24.0);
        self.text(rect.min, text, 1.5, rgb(0.89, 0.91, 0.94));
    }

    pub fn button(&mut self, id_source: &str, label: &str) -> bool {
        let id = self.scoped_id(id_source);
        let rect = self.next_rect(48.0);
        let hovered = rect.contains(self.input.mouse_pos);
        let pressed_here = self.input.press_pos.is_some_and(|pos| rect.contains(pos));
        let released_here = self.input.release_pos.is_some_and(|pos| rect.contains(pos));

        if hovered {
            self.memory.hot = Some(id);
        }
        if self.input.mouse_pressed && pressed_here {
            self.memory.active = Some(id);
        }

        let clicked = self.input.mouse_released && released_here && self.memory.active == Some(id);
        if self.input.mouse_released && self.memory.active == Some(id) {
            self.memory.active = None;
        }

        let color = if self.memory.active == Some(id) && self.input.mouse_down {
            rgb(0.93, 0.74, 0.45)
        } else if hovered {
            rgb(0.85, 0.81, 0.70)
        } else {
            rgb(0.78, 0.78, 0.75)
        };
        self.fill(rect, color);
        self.text(
            rect.min + Vec2::new(14.0, 15.0),
            label,
            1.8,
            rgb(0.08, 0.09, 0.11),
        );
        clicked
    }

    pub fn counter(&mut self, id_source: &str) -> i32 {
        self.memory.get_int(self.root_id(id_source))
    }

    pub fn bump_counter(&mut self, id_source: &str) {
        self.memory.bump(self.root_id(id_source));
    }

    fn begin_panel_with_rect(
        &mut self,
        id: u64,
        rect: Rect,
        padding: f32,
        spacing: f32,
        color: Color,
    ) {
        self.fill(rect, color);
        self.parents.push(Parent {
            id,
            cursor: rect.min + Vec2::splat(padding),
            width: rect.width() - padding * 2.0,
            spacing,
        });
    }

    fn next_rect(&mut self, height: f32) -> Rect {
        let parent = self.parents.last_mut().expect("panel required");
        let min = parent.cursor;
        let max = Vec2::new(min.x + parent.width, min.y + height);
        parent.cursor.y += height + parent.spacing;
        Rect::new(min, max)
    }

    fn scoped_id(&self, source: &str) -> u64 {
        let parent_id = self.parents.last().map(|parent| parent.id).unwrap_or(0);
        hash_pair(parent_id, source)
    }

    fn root_id(&self, source: &str) -> u64 {
        hash_pair(0, source)
    }
}

struct Parent {
    id: u64,
    cursor: Vec2,
    width: f32,
    spacing: f32,
}

fn hash_pair(seed: u64, source: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    source.hash(&mut hasher);
    hasher.finish()
}
