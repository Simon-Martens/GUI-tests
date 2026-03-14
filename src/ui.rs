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
    frame: u64,
    pub active: Option<u64>,
    ints: HashMap<u64, i32>,
    retained: HashMap<u64, RetainedWidget>,
}

impl UiMemory {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
    }

    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.retained
            .retain(|_, state| state.last_touched_frame == frame);

        if self
            .active
            .is_some_and(|id| !self.retained.contains_key(&id))
        {
            self.active = None;
        }
    }

    pub fn bump(&mut self, id: u64) {
        *self.ints.entry(id).or_insert(0) += 1;
    }

    pub fn get_int(&mut self, id: u64) -> i32 {
        *self.ints.entry(id).or_insert(0)
    }

    fn retained_widget(&mut self, id: u64) -> &mut RetainedWidget {
        let frame = self.frame;
        let state = self.retained.entry(id).or_default();
        state.last_touched_frame = frame;
        state
    }
}

pub struct Ui<'a> {
    memory: &'a mut UiMemory,
    input: &'a InputState,
    pub screen_size: Vec2,
    nodes: Vec<Node>,
    parents: Vec<usize>,
}

impl<'a> Ui<'a> {
    pub fn new(memory: &'a mut UiMemory, input: &'a InputState, screen_size: Vec2) -> Self {
        Self {
            memory,
            input,
            screen_size,
            nodes: Vec::new(),
            parents: Vec::new(),
        }
    }

    pub fn finish(mut self) -> Vec<DrawCmd> {
        let roots: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| node.parent.is_none().then_some(index))
            .collect();

        for &root in &roots {
            compute_size(&mut self.nodes, root);
        }
        for &root in &roots {
            let pos = self.nodes[root].origin;
            place_nodes(&mut self.nodes, root, pos);
        }

        let mut draw = Vec::new();
        for &root in &roots {
            emit_draws(&self.nodes, self.memory, root, &mut draw);
        }
        draw
    }

    pub fn fill(&mut self, rect: Rect, color: Color) {
        let index = self.push_node(NodeKind::Rect { color });
        self.nodes[index].origin = rect.min;
        self.nodes[index].size = Vec2::new(rect.width(), rect.max.y - rect.min.y);
        self.nodes[index].rect = rect;
    }

    pub fn text(&mut self, pos: Vec2, text: impl Into<String>, scale: f32, color: Color) {
        let text = text.into();
        let index = self.push_node(NodeKind::LooseText { text, scale, color });
        self.nodes[index].origin = pos;
    }

    pub fn begin_root_panel(
        &mut self,
        id_source: &str,
        pos: Vec2,
        padding: f32,
        spacing: f32,
        color: Color,
    ) {
        self.begin_panel(self.root_id(id_source), pos, padding, spacing, color);
    }

    pub fn end_panel(&mut self) {
        self.parents.pop();
    }

    pub fn label(&mut self, text: &str) {
        self.push_node(NodeKind::Label {
            text: text.to_string(),
            scale: 1.5,
            color: rgb(0.89, 0.91, 0.94),
        });
    }

    pub fn button(&mut self, id_source: &str, label: &str) -> bool {
        let id = self.scoped_id(id_source);
        let last_rect = self.memory.retained_widget(id).rect;
        let hovered = last_rect.is_some_and(|rect| rect.contains(self.input.mouse_pos));
        let pressed_here = self
            .input
            .press_pos
            .is_some_and(|pos| last_rect.is_some_and(|rect| rect.contains(pos)));
        let released_here = self
            .input
            .release_pos
            .is_some_and(|pos| last_rect.is_some_and(|rect| rect.contains(pos)));

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
        self.push_node(NodeKind::Button {
            id,
            label: label.to_string(),
            color,
            text_color: rgb(0.08, 0.09, 0.11),
        });
        clicked
    }

    pub fn counter(&mut self, id_source: &str) -> i32 {
        self.memory.get_int(self.root_id(id_source))
    }

    pub fn bump_counter(&mut self, id_source: &str) {
        self.memory.bump(self.root_id(id_source));
    }

    fn begin_panel(&mut self, id: u64, pos: Vec2, padding: f32, spacing: f32, color: Color) {
        let index = self.push_node(NodeKind::Panel {
            id,
            padding,
            spacing,
            color,
        });
        self.nodes[index].origin = pos;
        self.parents.push(index);
    }

    fn scoped_id(&self, source: &str) -> u64 {
        let parent_id = self
            .parents
            .last()
            .map(|&index| self.nodes[index].id())
            .unwrap_or(0);
        hash_pair(parent_id, source)
    }

    fn root_id(&self, source: &str) -> u64 {
        hash_pair(0, source)
    }

    fn push_node(&mut self, kind: NodeKind) -> usize {
        let parent = self.parents.last().copied();
        let index = self.nodes.len();
        self.nodes.push(Node {
            parent,
            children: Vec::new(),
            origin: Vec2::ZERO,
            size: Vec2::ZERO,
            rect: Rect::from_min_size(Vec2::ZERO, Vec2::ZERO),
            kind,
        });
        if let Some(parent) = parent {
            self.nodes[parent].children.push(index);
        }
        index
    }
}

struct Node {
    parent: Option<usize>,
    children: Vec<usize>,
    origin: Vec2,
    size: Vec2,
    rect: Rect,
    kind: NodeKind,
}

impl Node {
    fn id(&self) -> u64 {
        match self.kind {
            NodeKind::Panel { id, .. } => id,
            NodeKind::Button { id, .. } => id,
            _ => 0,
        }
    }
}

enum NodeKind {
    Rect {
        color: Color,
    },
    LooseText {
        text: String,
        scale: f32,
        color: Color,
    },
    Panel {
        id: u64,
        padding: f32,
        spacing: f32,
        color: Color,
    },
    Label {
        text: String,
        scale: f32,
        color: Color,
    },
    Button {
        id: u64,
        label: String,
        color: Color,
        text_color: Color,
    },
}

#[derive(Default)]
struct RetainedWidget {
    last_touched_frame: u64,
    rect: Option<Rect>,
}

fn hash_pair(seed: u64, source: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    source.hash(&mut hasher);
    hasher.finish()
}

fn compute_size(nodes: &mut [Node], index: usize) -> Vec2 {
    let children = nodes[index].children.clone();
    for child in children {
        compute_size(nodes, child);
    }

    let size = match &nodes[index].kind {
        NodeKind::Rect { .. } => nodes[index].size,
        NodeKind::LooseText { text, scale, .. } => text_size(text, *scale),
        NodeKind::Label { text, scale, .. } => text_size(text, *scale),
        NodeKind::Button { label, .. } => {
            let text = text_size(label, 1.8);
            Vec2::new(text.x + 28.0, text.y + 18.0)
        }
        NodeKind::Panel {
            padding, spacing, ..
        } => {
            let mut width: f32 = 0.0;
            let mut height: f32 = 0.0;
            for (child_i, child) in nodes[index].children.iter().copied().enumerate() {
                let child_size = nodes[child].size;
                width = width.max(child_size.x);
                height += child_size.y;
                if child_i > 0 {
                    height += *spacing;
                }
            }
            Vec2::new(width + padding * 2.0, height + padding * 2.0)
        }
    };
    nodes[index].size = size;
    size
}

fn place_nodes(nodes: &mut [Node], index: usize, origin: Vec2) {
    let size = nodes[index].size;
    nodes[index].rect = Rect::from_min_size(origin, size);

    if let NodeKind::Panel {
        padding, spacing, ..
    } = nodes[index].kind
    {
        let mut cursor = origin + Vec2::splat(padding);
        let children = nodes[index].children.clone();
        for child in children {
            place_nodes(nodes, child, cursor);
            cursor.y += nodes[child].size.y + spacing;
        }
    }
}

fn emit_draws(nodes: &[Node], memory: &mut UiMemory, index: usize, draw: &mut Vec<DrawCmd>) {
    match &nodes[index].kind {
        NodeKind::Rect { color } => draw.push(DrawCmd::Rect {
            rect: nodes[index].rect,
            color: *color,
        }),
        NodeKind::LooseText { text, scale, color } => draw.push(DrawCmd::Text {
            pos: nodes[index].rect.min,
            text: text.clone(),
            scale: *scale,
            color: *color,
        }),
        NodeKind::Panel { color, .. } => {
            draw.push(DrawCmd::Rect {
                rect: nodes[index].rect,
                color: *color,
            });
            for &child in &nodes[index].children {
                emit_draws(nodes, memory, child, draw);
            }
        }
        NodeKind::Label { text, scale, color } => draw.push(DrawCmd::Text {
            pos: nodes[index].rect.min,
            text: text.clone(),
            scale: *scale,
            color: *color,
        }),
        NodeKind::Button {
            id,
            label,
            color,
            text_color,
        } => {
            draw.push(DrawCmd::Rect {
                rect: nodes[index].rect,
                color: *color,
            });
            draw.push(DrawCmd::Text {
                pos: nodes[index].rect.min + Vec2::new(14.0, 15.0),
                text: label.clone(),
                scale: 1.8,
                color: *text_color,
            });
            memory.retained_widget(*id).rect = Some(nodes[index].rect);
        }
    }
}

fn text_size(text: &str, scale: f32) -> Vec2 {
    Vec2::new(text.chars().count() as f32 * 6.0 * scale, 7.0 * scale)
}
