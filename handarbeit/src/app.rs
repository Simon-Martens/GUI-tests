use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window as OsWindow, WindowAttributes, WindowId};

use crate::geom::{Point, Rect, Size, rgb};
use crate::gpu::{DrawCmd, GpuState};

pub fn run() {
    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("failed to run app");
}

#[derive(Default)]
struct App {
    window: Option<Arc<OsWindow>>,
    window_id: Option<WindowId>,
    gpu: Option<GpuState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("handarbeit")
            .with_inner_size(LogicalSize::new(760.0, 520.0))
            .with_min_inner_size(LogicalSize::new(500.0, 360.0));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create window failed"),
        );
        self.window_id = Some(window.id());
        // pollster: we block on an async operation, we can't use asyc here and new is an async function
        self.gpu =
            Some(pollster::block_on(GpuState::new(window.clone())).expect("init gpu failed"));
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
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    // this is run automatically, comes from the OS once no input arrives at the window level. We
    // manually request a redraw do trigger a WindowEvent::RedrawRequested, so the frames never
    // stop (fo now). TODO: draw only when necessary
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl App {
    // this is the actual drawing event. The drawing list will get tessellated into GPU primitives
    // to render.
    fn redraw(&mut self) {
        let window = match &self.window {
            Some(window) => window,
            None => return,
        };
        let gpu = match &mut self.gpu {
            Some(gpu) => gpu,
            None => return,
        };

        let size = window.inner_size();
        let draw_list = vec![
            DrawCmd::Rect {
                rect: Rect::from_origin_and_size(
                    Point::origin(),
                    Size::new(size.width as f32, size.height as f32),
                ),
                color: rgb(0.08, 0.09, 0.11),
            },
            DrawCmd::Rect {
                rect: Rect::from_origin_and_size(Point::new(36.0, 72.0), Size::new(96.0, 56.0)),
                color: rgb(0.82, 0.29, 0.24),
            },
            DrawCmd::Text {
                pos: Point::new(36.0, 144.0),
                text: "DRAWN FROM app.rs".to_string(),
                scale: 1.4,
                color: rgb(0.90, 0.92, 0.95),
                clip_rect: None,
            },
        ];

        match gpu.render(&draw_list) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.resize(window.inner_size())
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {}
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {}
        }
    }
}
