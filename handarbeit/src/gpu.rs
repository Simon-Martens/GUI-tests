use std::{error::Error, sync::Arc};

use winit::{dpi::PhysicalSize, window::Window};

pub struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl GpuState {
    // INFO: dyn == dynamic dispatch: we use a trait here as a reurn type, not a
    // concrete type, so we use dyn (all the returned errors have differing types)
    // Also, we return in a Box, bc we don't know the size of the error type at compile time.
    pub async fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
        let instance = wgpu::Instance::default();

        // INFO: a surface is a platform-specific object thats my rendering target
        let surface = instance.create_surface(window.clone())?;

        // INFO: a physical or logical choice of a GPU: we specify our needs and see if it fits
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                // INFO: we just use the default GPU/power preference,
                // but high performance is also possible, if we want the best GPU available.
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;

        // INFO: Finally or device and queue
        // DEVICE: create resources: buffers, shaders, textures, pipelines etc.
        // QUEUE: submit command buffers to the GPU for execution
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                ..Default::default()
            })
            .await?;

        // INFO: graphics cards have different capabilites and formats they support (eg. pixel
        // formats), so we query for this and set our formats accordingly. We might set this
        // differently, depending on the platform or our needs.
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);

        // INFO: here we configure the GPU/surface, inheriting from a default config
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("Surface does not support any compatible formats")?;
        config.present_mode = wgpu::PresentMode::AutoVsync; // request monitor dependent refresh rate
        config.format = format;
        config.alpha_mode = caps.alpha_modes[0];
        config.view_formats = vec![format];
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
        })
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }
}
