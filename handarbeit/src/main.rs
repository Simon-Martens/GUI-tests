mod gpu;

use crate::gpu::GpuState;
use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
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
            // INFO: here we draw, gets scheduled in about_to_wait
            WindowEvent::RedrawRequested => {}
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
