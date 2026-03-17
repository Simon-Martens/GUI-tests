use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window as OsWindow, WindowAttributes, WindowId};

use crate::geom::{Point, Size};
use crate::gpu::GpuState;
use crate::ui::{InputState, Render, UiAction, UiMemory, Window};

fn elapsed_ms(start: Instant, end: Instant) -> f64 {
    end.duration_since(start).as_secs_f64() * 1000.0
}

#[derive(Clone, Copy, Default)]
pub struct DebugOptions {
    pub time_frames: bool,
}

pub fn run<V: Render>(view: V, debug: DebugOptions) {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new(view, debug);
    event_loop.run_app(&mut app).expect("failed to run app");
}

struct App<V: Render> {
    window: Option<Arc<OsWindow>>,
    window_id: Option<WindowId>,
    gpu: Option<GpuState>,
    input: InputState,
    memory: UiMemory,
    frame_number: u64,
    debug: DebugOptions,
    view: V,
}

impl<V: Render> App<V> {
    fn new(view: V, debug: DebugOptions) -> Self {
        Self {
            window: None,
            window_id: None,
            gpu: None,
            input: InputState::default(),
            memory: UiMemory::default(),
            frame_number: 0,
            debug,
            view,
        }
    }
}

impl<V: Render> ApplicationHandler for App<V> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("GPUI-shaped immediate UI")
            .with_inner_size(LogicalSize::new(760.0, 520.0))
            .with_min_inner_size(LogicalSize::new(500.0, 360.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        self.window_id = Some(window.id());
        self.gpu = Some(pollster::block_on(GpuState::new(window.clone())).expect("init gpu"));
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(gpu), Some(window)) = (&mut self.gpu, &self.window) {
                    gpu.resize(window.inner_size());
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = Point::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    self.input.mouse_down = true;
                    self.input.mouse_pressed = true;
                    self.input.press_pos = Some(self.input.mouse_pos);
                }
                ElementState::Released => {
                    self.input.mouse_down = false;
                    self.input.mouse_released = true;
                    self.input.release_pos = Some(self.input.mouse_pos);
                }
            },
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl<V: Render> App<V> {
    fn redraw(&mut self) {
        let window = match &self.window {
            Some(window) => window.clone(),
            None => return,
        };
        if self.gpu.is_none() {
            return;
        }

        self.frame_number += 1;
        let frame_start = self.debug.time_frames.then(Instant::now);
        self.memory.begin_frame();

        let size = window.inner_size();
        let mut ui_window = Window::new(
            &mut self.memory,
            &self.input,
            Size::new(size.width as f32, size.height as f32),
        );
        let draw_result = ui_window.draw(&mut self.view, self.debug.time_frames);
        let after_draw = self.debug.time_frames.then(Instant::now);
        let draw_timing = draw_result.timing;
        let output = draw_result.output;
        let _interaction = output.interaction;
        let _clicked = output.interaction.clicked;

        for action in output.actions {
            self.apply_action(action);
        }
        self.memory.end_frame();
        let after_actions = self.debug.time_frames.then(Instant::now);

        let gpu = match &mut self.gpu {
            Some(gpu) => gpu,
            None => return,
        };
        let render_timing = match gpu.render(&output.draw_list, self.debug.time_frames) {
            Ok(timing) => timing,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.resize(window.inner_size());
                None
            }
            Err(wgpu::SurfaceError::OutOfMemory) => return,
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => None,
        };
        let after_render = self.debug.time_frames.then(Instant::now);
        self.input.end_frame();
        let frame_end = self.debug.time_frames.then(Instant::now);

        if let (
            Some(frame_start),
            Some(after_draw),
            Some(draw_timing),
            Some(after_actions),
            Some(render_timing),
            Some(after_render),
            Some(frame_end),
        ) = (
            frame_start,
            after_draw,
            draw_timing,
            after_actions,
            render_timing,
            after_render,
            frame_end,
        )
        {
            eprintln!(
                "frame {}: draw(render={:.3} ms prepaint={:.3} ms interact={:.3} ms paint={:.3} ms total={:.3} ms) actions={:.3} ms render(acquire={:.3} ms tessellate={:.3} ms upload={:.3} ms encode={:.3} ms submit_present={:.3} ms total={:.3} ms) finish={:.3} ms total={:.3} ms",
                self.frame_number,
                draw_timing.render_tree_ms,
                draw_timing.prepaint_ms,
                draw_timing.interaction_ms,
                draw_timing.paint_ms,
                draw_timing.total_ms,
                elapsed_ms(after_draw, after_actions),
                render_timing.acquire_ms,
                render_timing.tessellate_ms,
                render_timing.upload_ms,
                render_timing.encode_ms,
                render_timing.submit_present_ms,
                render_timing.total_ms,
                elapsed_ms(after_render, frame_end),
                elapsed_ms(frame_start, frame_end),
            );
        }
    }

    fn apply_action(&mut self, action: UiAction) {
        match action {
            UiAction::Clicked(id) => {
                let _ = id;
            }
            UiAction::BumpInt(id) => self.memory.bump(id),
        }
    }
}
