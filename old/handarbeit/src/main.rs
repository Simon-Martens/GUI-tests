mod geom;
mod gpu;
mod text;
mod ui;

use crate::geom::{Rect, Vec2, rgb};
use crate::gpu::GpuState;
use crate::ui::{InputState, Ui, UiMemory};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event::ElementState,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

fn main() {
    run();
}

fn run() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop
        .run_app(&mut app)
        .expect("Failed to run application");
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    window_id: Option<WindowId>,
    pgpu: Option<GpuState>,
    input: InputState,
    memory: UiMemory,
}

impl ApplicationHandler for App {
    // INFO: this is called as soon as the application becomes active (again)
    // and can create windows. It is also called once at the start of the app.
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let atts = WindowAttributes::default()
            .with_title("Hello world")
            .with_inner_size(LogicalSize::new(800.0, 600.0));

        let window = Arc::new(
            event_loop
                .create_window(atts)
                .expect("Failed to create window"),
        );

        self.window_id = Some(window.id());
        self.pgpu = Some(
            // INFO: since resumed is a hook from winit, we can't change the function definition to
            // be async. So we use pollster to just block here until the async function finishes.
            // Pollster lays the thread to sleep, polls the future and hands over execution back to
            // the thread once the future is fulfilled.
            pollster::block_on(GpuState::new(window.clone())).expect("Failed to initialize GPU"),
        );
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        // INFO: bc we have multiple windows
        if Some(window_id) != self.window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.pgpu {
                    gpu.resize(size);
                }
            }
            // INFO: here we draw, gets scheduled in about_to_wait
            WindowEvent::RedrawRequested => {
                self.memory.begin_frame();

                if let Some(gpu) = &mut self.pgpu {
                    // INFO: we recreate the UI here on every single frame
                    let size = if let Some(window) = &self.window {
                        window.inner_size()
                    } else {
                        winit::dpi::PhysicalSize::new(800, 600)
                    };
                    let mut ui = Ui::new(
                        &mut self.memory,
                        &self.input,
                        Vec2::new(size.width as f32, size.height as f32),
                    );
                    ui.fill(
                        Rect::from_min_size(Vec2::new(0.0, 0.0), ui.screen_size),
                        rgb(0.1, 0.2, 0.3),
                    );
                    ui.text(
                        Vec2::new(40.0, 40.0),
                        "Handarbeit UI",
                        1.8,
                        rgb(0.85, 0.88, 0.92),
                    );
                    let click_count = ui.counter("button_count");
                    ui.begin_root_panel(
                        "panel",
                        Vec2::new(100.0, 80.0),
                        18.0,
                        12.0,
                        rgb(0.18, 0.20, 0.24),
                    );
                    ui.label("Width From Children");
                    if ui.button("button", format!("Clicks {}", click_count)) {
                        ui.bump_counter("button_count");
                    }
                    ui.end_panel();
                    let draw_list = ui.finish();

                    match gpu.render(&draw_list) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            if let Some(window) = &self.window {
                                gpu.resize(window.inner_size());
                            }
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            event_loop.exit();
                        }
                        Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {}
                    }
                }

                self.memory.end_frame();
                self.input.end_frame();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(mouse) = self.input.mouse_button_mut(button) {
                    mouse.set(state == ElementState::Pressed);
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // INFO: runs on the end of a frame
        // INFO: do not sleep here, while we have no events, poll the loop
        // normally it could be: while (true) { update(); render(); }
        event_loop.set_control_flow(ControlFlow::Poll);

        if let Some(window) = &self.window {
            // INFO: does not redraw here, schedules a redraw with RedrawRequested
            // Basically schedules the next frame render
            window.request_redraw();
        }
    }
}
