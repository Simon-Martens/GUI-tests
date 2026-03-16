use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window as OsWindow, WindowAttributes, WindowId};

use crate::geom::Vec2;
use crate::gpu::GpuState;
use crate::ui::{InputState, Render, UiAction, UiMemory, Window};

pub fn run<V: Render>(view: V) {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new(view);
    event_loop.run_app(&mut app).expect("failed to run app");
}

struct App<V: Render> {
    window: Option<Arc<OsWindow>>,
    window_id: Option<WindowId>,
    gpu: Option<GpuState>,
    input: InputState,
    memory: UiMemory,
    frames: u64,
    view: V,
}

impl<V: Render> App<V> {
    fn new(view: V) -> Self {
        Self {
            window: None,
            window_id: None,
            gpu: None,
            input: InputState::default(),
            memory: UiMemory::default(),
            frames: 0,
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
                self.input.mouse_pos = Vec2::new(position.x as f32, position.y as f32);
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

        self.frames += 1;
        self.memory.begin_frame();

        let size = window.inner_size();
        let mut ui_window = Window::new(
            &mut self.memory,
            &self.input,
            Vec2::new(size.width as f32, size.height as f32),
            self.frames,
        );
        let output = ui_window.draw(&mut self.view);
        let _interaction = output.interaction;
        let _clicked = output.interaction.clicked;

        for action in output.actions {
            self.apply_action(action);
        }
        self.memory.end_frame();

        let gpu = match &mut self.gpu {
            Some(gpu) => gpu,
            None => return,
        };
        match gpu.render(&output.draw_list) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.resize(window.inner_size())
            }
            Err(wgpu::SurfaceError::OutOfMemory) => return,
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {}
        }
        self.input.end_frame();
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
