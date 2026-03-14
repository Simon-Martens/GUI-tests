use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::geom::{Rect, Vec2, rgb};
use crate::gpu::GpuState;
use crate::ui::{InputState, Ui, UiMemory};

pub fn run() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("failed to run app");
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    window_id: Option<WindowId>,
    gpu: Option<GpuState>,
    input: InputState,
    memory: UiMemory,
    frames: u64,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("Immediate Mode Parent Stack")
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

impl App {
    fn redraw(&mut self) {
        let (gpu, window) = match (&mut self.gpu, &self.window) {
            (Some(gpu), Some(window)) => (gpu, window),
            _ => return,
        };

        self.frames += 1;
        self.memory.begin_frame();

        let size = window.inner_size();
        let mut ui = Ui::new(
            &mut self.memory,
            &self.input,
            Vec2::new(size.width as f32, size.height as f32),
        );
        build_demo(&mut ui, self.frames);
        let draw_list = ui.finish();
        self.memory.end_frame();

        match gpu.render(&draw_list) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.resize(window.inner_size())
            }
            Err(wgpu::SurfaceError::OutOfMemory) => return,
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {}
        }
        self.input.end_frame();
    }
}

fn build_demo(ui: &mut Ui<'_>, frames: u64) {
    let screen = Rect::from_min_size(Vec2::ZERO, ui.screen_size);
    ui.fill(screen, rgb(0.08, 0.09, 0.11));
    ui.text(
        Vec2::new(18.0, 18.0),
        format!("FRAMES {frames}"),
        1.6,
        rgb(0.76, 0.80, 0.84),
    );

    let count = ui.counter("button_count");
    ui.begin_root_panel(
        "panel",
        Vec2::new(
            ui.screen_size.x * 0.5 - 110.0,
            ui.screen_size.y * 0.5 - 55.0,
        ),
        18.0,
        12.0,
        rgb(0.14, 0.16, 0.20),
    );
    ui.label("WIDTH FROM CHILDREN");
    if ui.button("button", &format!("BUTTON {count}")) {
        ui.bump_counter("button_count");
    }
    ui.end_panel();
}
