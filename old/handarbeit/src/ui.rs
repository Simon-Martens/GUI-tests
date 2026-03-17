use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::geom::{Rect, Vec2, rgb};
use crate::gpu::DrawCmd;
use crate::text;
use winit::event::MouseButton;

#[derive(Default, Clone, Copy)]
pub struct ButtonState {
    // INFO: true if button is currently pressed
    pub down: bool,
    // INFO: true if button down state started this frame (down: false -> true)
    pub pressed: bool,
    // INFO: true if button down state ended this frame (down: true -> false)
    pub released: bool,
}

impl ButtonState {
    pub fn end_frame(&mut self) {
        self.pressed = false;
        self.released = false;
    }

    pub fn set(&mut self, down: bool) {
        if down && !self.down {
            self.pressed = true;
        } else if !down && self.down {
            self.released = true;
        }

        self.down = down;
    }
}

#[derive(Default)]
pub struct InputState {
    pub mouse_pos: Vec2,
    pub left_mouse: ButtonState,
    pub right_mouse: ButtonState,
}

impl InputState {
    pub fn end_frame(&mut self) {
        self.left_mouse.end_frame();
        self.right_mouse.end_frame();
    }

    pub fn mouse_button_mut(&mut self, button: MouseButton) -> Option<&mut ButtonState> {
        match button {
            MouseButton::Left => Some(&mut self.left_mouse),
            MouseButton::Right => Some(&mut self.right_mouse),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct UiMemory {
    frame: u64,
    pub active: Option<u64>,
    ints: HashMap<u64, i32>,
    widget_map: HashMap<u64, usize>,
    widgets: Vec<Widget>,
}

impl UiMemory {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
    }

    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.widget_map.clear();

        let mut write = 0;
        let len = self.widgets.len();
        for read in 0..len {
            if self.widgets[read].last_frame_touched != frame {
                continue;
            }

            if write != read {
                self.widgets.swap(write, read);
            }

            let id = self.widgets[write].id;
            self.widget_map.insert(id, write);
            write += 1;
        }

        self.widgets.truncate(write);

        if self
            .active
            .is_some_and(|id| !self.widget_map.contains_key(&id))
        {
            self.active = None;
        }
    }

    fn root_id(&self, source: &str) -> u64 {
        hash_pair(0, source)
    }

    fn widget(&mut self, id: u64) -> &mut Widget {
        let frame = self.frame;
        let index = if let Some(&index) = self.widget_map.get(&id) {
            index
        } else {
            let index = self.widgets.len();
            self.widgets.push(Widget {
                id,
                ..Default::default()
            });
            self.widget_map.insert(id, index);
            index
        };
        let widget = &mut self.widgets[index];
        widget.last_frame_touched = frame;
        widget
    }

    fn bump(&mut self, id: u64) {
        *self.ints.entry(id).or_insert(0) += 1;
    }

    fn get_int(&mut self, id: u64) -> i32 {
        *self.ints.entry(id).or_insert(0)
    }
}

#[derive(Default)]
struct Widget {
    id: u64,
    last_frame_touched: u64,
    rect: Option<Rect>,
    kind: WidgetKind,
    text: String,
    scale: f32,
    color: [f32; 4],
    text_color: [f32; 4],
    padding: f32,
    spacing: f32,
}

pub struct Ui<'a> {
    memory: &'a mut UiMemory,
    input: &'a InputState,
    pub screen_size: Vec2,
    // TODO: We need to reuse these buffers so no allocations in the render path
    // This is the backing store for the tree of widgets
    nodes: Vec<Node>,
    parents: Vec<usize>,
    next_auto_id: u64,
}

impl<'a> Ui<'a> {
    pub fn new(memory: &'a mut UiMemory, input: &'a InputState, screen_size: Vec2) -> Self {
        Self {
            memory,
            input,
            screen_size,
            nodes: Vec::new(),
            parents: Vec::new(),
            next_auto_id: 0,
        }
    }

    // TODO: we do arena allocations here if possible
    pub fn button(&mut self, id_source: &str, label: impl Into<String>) -> bool {
        let id = self.scoped_id(id_source);
        let widget_index = self.memory.widget_map.get(&id).copied();
        let last_rect = widget_index.and_then(|index| self.memory.widgets[index].rect);
        let hovered = last_rect.is_some_and(|rect| rect.contains(self.input.mouse_pos));

        if self.input.left_mouse.pressed && hovered {
            self.memory.active = Some(id);
        }

        let clicked = self.input.left_mouse.released && hovered && self.memory.active == Some(id);

        if self.input.left_mouse.released && self.memory.active == Some(id) {
            self.memory.active = None;
        }

        let color = if self.memory.active == Some(id) && self.input.left_mouse.down {
            rgb(0.93, 0.74, 0.45)
        } else if hovered {
            rgb(0.95, 0.82, 0.45)
        } else {
            rgb(0.9, 0.7, 0.3)
        };

        let label = label.into();
        let widget = self.memory.widget(id);
        widget.kind = WidgetKind::Button;
        widget.text.clear();
        widget.text.push_str(&label);
        widget.scale = 1.8;
        widget.color = color;
        widget.text_color = rgb(0.08, 0.09, 0.11);
        let widget_index = self.memory.widget_map[&id];

        self.push_node(Node {
            widget_index,
            parent: None,
            children: Vec::new(),
            origin: Vec2::ZERO,
            size: Vec2::ZERO,
            rect: Rect::from_min_size(Vec2::ZERO, Vec2::ZERO),
        });

        clicked
    }

    pub fn fill(&mut self, rect: Rect, color: [f32; 4]) {
        let id = self.auto_id(1);
        let widget = self.memory.widget(id);
        widget.kind = WidgetKind::Rect;
        widget.color = color;
        let widget_index = self.memory.widget_map[&id];
        self.push_node(Node {
            widget_index,
            parent: None,
            children: Vec::new(),
            origin: rect.min,
            size: Vec2::new(rect.width(), rect.height()),
            rect,
        });
    }

    pub fn text(&mut self, pos: Vec2, text: impl Into<String>, scale: f32, color: [f32; 4]) {
        let text = text.into();
        let id = self.auto_id(2);
        let widget = self.memory.widget(id);
        widget.kind = WidgetKind::LooseText;
        widget.text.clear();
        widget.text.push_str(&text);
        widget.scale = scale;
        widget.color = color;
        let widget_index = self.memory.widget_map[&id];
        self.push_node(Node {
            widget_index,
            parent: None,
            children: Vec::new(),
            origin: pos,
            size: Vec2::ZERO,
            rect: Rect::from_min_size(pos, Vec2::ZERO),
        });
    }

    pub fn begin_root_panel(
        &mut self,
        id_source: &str,
        pos: Vec2,
        padding: f32,
        spacing: f32,
        color: [f32; 4],
    ) {
        let id = self.memory.root_id(id_source);
        let widget = self.memory.widget(id);
        widget.kind = WidgetKind::Panel;
        widget.color = color;
        widget.padding = padding;
        widget.spacing = spacing;
        let widget_index = self.memory.widget_map[&id];
        let index = self.push_node(Node {
            widget_index,
            parent: None,
            children: Vec::new(),
            origin: pos,
            size: Vec2::ZERO,
            rect: Rect::from_min_size(pos, Vec2::ZERO),
        });
        self.parents.push(index);
    }

    pub fn label(&mut self, text: impl Into<String>) {
        let text = text.into();
        let id = self.auto_id(3);
        let widget = self.memory.widget(id);
        widget.kind = WidgetKind::Label;
        widget.text.clear();
        widget.text.push_str(&text);
        widget.scale = 1.5;
        widget.color = rgb(0.89, 0.91, 0.94);
        let widget_index = self.memory.widget_map[&id];
        self.push_node(Node {
            widget_index,
            parent: None,
            children: Vec::new(),
            origin: Vec2::ZERO,
            size: Vec2::ZERO,
            rect: Rect::from_min_size(Vec2::ZERO, Vec2::ZERO),
        });
    }

    pub fn end_panel(&mut self) {
        self.parents.pop();
    }

    pub fn counter(&mut self, id_source: &str) -> i32 {
        self.memory.get_int(self.memory.root_id(id_source))
    }

    pub fn bump_counter(&mut self, id_source: &str) {
        self.memory.bump(self.memory.root_id(id_source));
    }

    pub fn finish(mut self) -> Vec<DrawCmd> {
        let roots: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| node.parent.is_none().then_some(index))
            .collect();

        for &root in &roots {
            compute_size(&self.memory.widgets, &mut self.nodes, root);
        }
        for &root in &roots {
            let origin = self.nodes[root].origin;
            place_nodes(&self.memory.widgets, &mut self.nodes, root, origin);
        }
        for node in &self.nodes {
            self.memory.widgets[node.widget_index].rect = Some(node.rect);
        }

        let mut draw_list = Vec::new();
        for &root in &roots {
            emit_draws(&self.memory.widgets, &self.nodes, root, &mut draw_list);
        }
        draw_list
    }

    fn push_node(&mut self, mut node: Node) -> usize {
        let parent = self.parents.last().copied();
        node.parent = parent;
        let index = self.nodes.len();
        self.nodes.push(node);
        if let Some(parent) = parent {
            self.nodes[parent].children.push(index);
        }
        index
    }

    fn auto_id(&mut self, tag: u64) -> u64 {
        let parent_id = self
            .parents
            .last()
            .map(|&index| self.memory.widgets[self.nodes[index].widget_index].id)
            .unwrap_or(0);
        let ordinal = self.next_auto_id;
        self.next_auto_id += 1;
        hash_u64(hash_u64(parent_id, tag), ordinal)
    }

    fn scoped_id(&self, source: &str) -> u64 {
        let parent_id = self
            .parents
            .last()
            .map(|&index| self.memory.widgets[self.nodes[index].widget_index].id)
            .unwrap_or(0);
        hash_pair(parent_id, source)
    }
}

struct Node {
    widget_index: usize,
    parent: Option<usize>,
    children: Vec<usize>,
    origin: Vec2,
    size: Vec2,
    rect: Rect,
}

#[derive(Default)]
enum WidgetKind {
    #[default]
    None,
    Panel,
    Rect,
    LooseText,
    Label,
    Button,
}

fn compute_size(widgets: &[Widget], nodes: &mut [Node], index: usize) -> Vec2 {
    let children = nodes[index].children.clone();
    for child in children {
        compute_size(widgets, nodes, child);
    }

    let widget = &widgets[nodes[index].widget_index];
    let size = match widget.kind {
        WidgetKind::None => Vec2::ZERO,
        WidgetKind::Rect => nodes[index].size,
        WidgetKind::LooseText | WidgetKind::Label => text::measure(&widget.text, widget.scale),
        WidgetKind::Button => {
            let text = text::measure(&widget.text, widget.scale);
            Vec2::new(text.x + 28.0, text.y + 18.0)
        }
        WidgetKind::Panel => {
            let mut width: f32 = 0.0;
            let mut height: f32 = 0.0;
            for (child_i, child) in nodes[index].children.iter().copied().enumerate() {
                let child_size = nodes[child].size;
                width = width.max(child_size.x);
                height += child_size.y;
                if child_i > 0 {
                    height += widget.spacing;
                }
            }
            Vec2::new(width + widget.padding * 2.0, height + widget.padding * 2.0)
        }
    };
    nodes[index].size = size;
    size
}

fn place_nodes(widgets: &[Widget], nodes: &mut [Node], index: usize, origin: Vec2) {
    nodes[index].origin = origin;
    nodes[index].rect = Rect::from_min_size(origin, nodes[index].size);

    let widget = &widgets[nodes[index].widget_index];
    let children = nodes[index].children.clone();
    if matches!(widget.kind, WidgetKind::Panel) {
        let mut cursor = origin + Vec2::splat(widget.padding);
        for child in children {
            place_nodes(widgets, nodes, child, cursor);
            cursor.y += nodes[child].size.y + widget.spacing;
        }
    }
}

fn emit_draws(widgets: &[Widget], nodes: &[Node], index: usize, draw_list: &mut Vec<DrawCmd>) {
    let node = &nodes[index];
    let widget = &widgets[node.widget_index];
    match &widget.kind {
        WidgetKind::None => {}
        WidgetKind::Panel => {
            draw_list.push(DrawCmd::Rect {
                rect: node.rect,
                color: widget.color,
            });
        }
        WidgetKind::Rect => {
            draw_list.push(DrawCmd::Rect {
                rect: node.rect,
                color: widget.color,
            });
        }
        WidgetKind::LooseText => {
            draw_list.push(DrawCmd::Text {
                pos: node.rect.min,
                text: widget.text.clone(),
                scale: widget.scale,
                color: widget.color,
            });
        }
        WidgetKind::Label => {
            draw_list.push(DrawCmd::Text {
                pos: node.rect.min,
                text: widget.text.clone(),
                scale: widget.scale,
                color: widget.color,
            });
        }
        WidgetKind::Button => {
            let text_size = text::measure(&widget.text, widget.scale);
            let text_pos = Vec2::new(
                node.rect.min.x + (node.rect.width() - text_size.x) * 0.5,
                node.rect.min.y + (node.rect.height() - text_size.y) * 0.5,
            );
            draw_list.push(DrawCmd::Rect {
                rect: node.rect,
                color: widget.color,
            });
            draw_list.push(DrawCmd::Text {
                pos: text_pos,
                text: widget.text.clone(),
                scale: widget.scale,
                color: widget.text_color,
            });
        }
    }

    for &child in &node.children {
        emit_draws(widgets, nodes, child, draw_list);
    }
}

fn hash_pair(seed: u64, source: &str) -> u64 {
    // TODO: why do we create a hasher every time we generate a hash for the thing?
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    source.hash(&mut hasher);
    hasher.finish()
}

fn hash_u64(seed: u64, value: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    value.hash(&mut hasher);
    hasher.finish()
}
