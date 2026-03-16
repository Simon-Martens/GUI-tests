mod geom;
mod gpu;
mod text;

use crate::geom::{Rect, Vec2, rgb};
use crate::gpu::{DrawCmd, GpuState};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event::{ElementState, MouseButton},
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
    button_active: bool,
    click_count: u32,
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
                if let Some(gpu) = &mut self.pgpu {
                    // INFO: drawing happpens through our own control language.
                    let button_rect =
                        Rect::from_min_size(Vec2::new(120.0, 100.0), Vec2::new(220.0, 140.0));
                    let hovered = button_rect.contains(self.input.mouse_pos);

                    if self.input.mouse_pressed && hovered {
                        self.button_active = true;
                    }

                    let clicked = self.input.mouse_released && hovered && self.button_active;

                    if self.input.mouse_released {
                        self.button_active = false;
                    }

                    if clicked {
                        self.click_count += 1;
                    }

                    let button_color = if self.button_active && self.input.mouse_down {
                        rgb(0.93, 0.74, 0.45)
                    } else if hovered {
                        rgb(0.95, 0.82, 0.45)
                    } else {
                        rgb(0.9, 0.7, 0.3)
                    };

                    // This is our layout or at least our DrawList
                    let draw_list = [
                        DrawCmd::Rect {
                            rect: button_rect,
                            color: button_color,
                        },
                        DrawCmd::Text {
                            pos: button_rect.min + Vec2::new(30.0, 45.0),
                            text: format!("Clicks {}", self.click_count),
                            scale: 1.8,
                            color: rgb(0.08, 0.09, 0.11),
                        },
                    ];

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

                self.input.end_frame();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            self.input.mouse_down = true;
                            self.input.mouse_pressed = true;
                        }
                        ElementState::Released => {
                            self.input.mouse_down = false;
                            self.input.mouse_released = true;
                        }
                    }
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

#[derive(Default)]
struct InputState {
    mouse_pos: Vec2,
    mouse_down: bool,
    mouse_pressed: bool,
    mouse_released: bool,
}

impl InputState {
    fn end_frame(&mut self) {
        self.mouse_pressed = false;
        self.mouse_released = false;
    }
}
