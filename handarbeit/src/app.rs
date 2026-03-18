use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window as OsWindow, WindowAttributes, WindowId};

use crate::geom::{Point, Size};
use crate::gpu::GpuState;
use crate::ui::{InputState, Render, UiMemory, Window};

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
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = Point::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => {
                    if !self.input.mouse_down {
                        self.input.mouse_down = true;
                        self.input.mouse_pressed = true;
                        self.input.press_pos = Some(self.input.mouse_pos);
                    }
                }
                ElementState::Released => {
                    self.input.mouse_down = false;
                    self.input.mouse_released = true;
                    self.input.release_pos = Some(self.input.mouse_pos);
                }
            },
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

impl<V: Render> App<V> {
    // this is the actual drawing event. The drawing list will get tessellated into GPU primitives
    // to render.
    fn redraw(&mut self) {
        let window = match &self.window {
            Some(window) => window.clone(),
            None => return,
        };
        if self.gpu.is_none() {
            return;
        }

        let size = window.inner_size();
        self.memory.begin_frame();
        // Window is frame-loacal data to pass to the render fucntion:
        // - input state
        // - window data (currently just wxh)
        // - the memory of ui elements of last state
        // - a layout tree
        // TODO: see that we can reuse old windows, not having to allocate again
        let mut ui_window = Window::new(
            &mut self.memory,
            &self.input,
            Size::new(size.width as f32, size.height as f32),
        );
        let mut root = self.view.render(&mut ui_window);
        root.prepaint_as_root(Point::origin(), ui_window.screen_size(), &mut ui_window);
        ui_window.resolve_frame_interaction();
        root.paint(&mut ui_window);
        let draw_list = ui_window.finish();
        self.memory.end_frame();
        self.input.end_frame();

        let gpu = match &mut self.gpu {
            Some(gpu) => gpu,
            None => return,
        };

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
