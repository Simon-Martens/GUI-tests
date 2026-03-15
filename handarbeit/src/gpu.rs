use crate::geom::{Color, Rect, to_ndc};
use bytemuck::{Pod, Zeroable};
use std::{error::Error, sync::Arc};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

// INFO: these are the vertex/fragment shaders, they get compiled an run on the GPU
const SHADER: &str = r#"
struct VertexInput {
  @location(0) position: vec2<f32>,
  @location(1) color: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
  var output: VertexOutput;
  output.clip_position = vec4<f32>(input.position, 0.0, 1.0);
  output.color = input.color;
  return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color;
}
"#;

pub enum DrawCmd {
    Rect { rect: Rect, color: Color },
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    fn new(position: [f32; 2], color: Color) -> Self {
        Self { position, color }
    }

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        // INFO: this is the Vertex struct above as described for wgpu
        // or in effect for the GPU, this will become vertex shader input.
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
            ],
        }
    }
}

// INFO: we create triangles for opur GPU to render from our draw commands, which edges will make up
// the vertices that will be inpout into the vertex shader later. This is called tessellation.
fn tessellate(draw_list: &[DrawCmd], width: f32, height: f32) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    for cmd in draw_list {
        match cmd {
            DrawCmd::Rect { rect, color } => {
                push_rect(&mut vertices, *rect, *color, width, height);
            }
        }
    }

    vertices
}

fn push_rect(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, width: f32, height: f32) {
    let min = to_ndc(rect.min, width, height);
    let max = to_ndc(rect.max, width, height);

    // INFO: we push 6 vertices for the 2 triangles that make up the rectangle, in a specific order
    vertices.extend_from_slice(&[
        Vertex::new([min.x, min.y], color),
        Vertex::new([min.x, max.y], color),
        Vertex::new([max.x, min.y], color),
        Vertex::new([max.x, min.y], color),
        Vertex::new([min.x, max.y], color),
        Vertex::new([max.x, max.y], color),
    ]);
}

pub struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
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

    pub fn render(&mut self, draw_list: &[DrawCmd]) -> Result<(), wgpu::SurfaceError> {
        // INFO: when width and height is 0, it's customary to skip rendering.
        // It's oftne the case when a window is minimized, or invisible for some reason
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(());
        }

        // INFO: this is the texture that is ready for rendering into. The texture is owned
        // by the GPU, it is just a 2D representation of the image; down blow it is pixel
        // data in a specific format with some properties and flags.
        // The rasterizer will write into this buffer.
        let output = self.surface.get_current_texture()?;
        // gives the program the methods/formats to render into the texture, by specifing a
        // description of the texture we want to use.
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // INFO: we get the vertices to render
        let vertices = tessellate(
            draw_list,
            self.config.width as f32,
            self.config.height as f32,
        );
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // INFO: here the encoder records all commands we want to execute on the GPU, so we can send
        // them in a batch, so we record them in a command buffer. So the encoder is kinda like a
        // buffer with methods attchhed to write commands into it.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear encoder"),
            });

        {
            // INFO: this is the render pass
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    // Stuff for multisampling, will use later
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // INFO: Load & Store => what to do with the former contents of the texture,
                        // and what to do with the new contentrs after rendering. We first clear the
                        // texture to a specific color, and then use this texture later when
                        // displaying it on the screen, so we store it.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    // INFO: this is for 3D textures, or layered textutures
                    depth_slice: None,
                })],
                // Near/far away objects description
                depth_stencil_attachment: None,
                // Is an object visible or not? Very useful for optimization
                occlusion_query_set: None,
                // needed for GPU profiling
                timestamp_writes: None,
                // Multiple views of the same texture in one pass, like for VR or stereoscopic
                // rendering, but we don't need it here
                multiview_mask: None,
            });

            // INFO: now we render our vertex buffer into the solid background
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.queue.submit([encoder.finish()]); // INFO: submitting the commands to the GPU
        output.present();
        // INFO: finallly, we display the image, or better: submit for presentation
        // This might wait for the appropriate time (eg. if VSYNC is enabled)
        return Ok(());
    }
}
