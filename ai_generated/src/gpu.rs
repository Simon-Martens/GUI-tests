use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::geom::{Color, Rect, Vec2, to_ndc};
use crate::text;

fn elapsed_ms(start: Instant, end: Instant) -> f64 {
    end.duration_since(start).as_secs_f64() * 1000.0
}

pub enum DrawCmd {
    Rect {
        rect: Rect,
        color: Color,
    },
    Text {
        pos: Vec2,
        text: String,
        scale: f32,
        color: Color,
        clip_rect: Option<Rect>,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderTiming {
    pub acquire_ms: f64,
    pub tessellate_ms: f64,
    pub upload_ms: f64,
    pub encode_ms: f64,
    pub submit_present_ms: f64,
    pub total_ms: f64,
}

pub struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
}

impl GpuState {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| format!("create surface: {err}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|err| format!("request adapter: {err}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                ..Default::default()
            })
            .await
            .map_err(|err| format!("request device: {err}"))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| "surface not supported by adapter".to_string())?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
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
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
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
            multiview: None,
            cache: None,
        });

        Ok(Self {
            window,
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

    pub fn render(
        &mut self,
        draw_list: &[DrawCmd],
        capture_timing: bool,
    ) -> Result<Option<RenderTiming>, wgpu::SurfaceError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(None);
        }

        let render_start = capture_timing.then(Instant::now);
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let after_acquire = capture_timing.then(Instant::now);
        let vertices = tessellate(
            draw_list,
            self.config.width as f32,
            self.config.height as f32,
        );
        let after_tessellate = capture_timing.then(Instant::now);
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let after_upload = capture_timing.then(Instant::now);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.09,
                            b: 0.11,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        let after_encode = capture_timing.then(Instant::now);

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        output.present();
        let after_submit_present = capture_timing.then(Instant::now);

        let timing = if let (
            Some(render_start),
            Some(after_acquire),
            Some(after_tessellate),
            Some(after_upload),
            Some(after_encode),
            Some(after_submit_present),
        ) = (
            render_start,
            after_acquire,
            after_tessellate,
            after_upload,
            after_encode,
            after_submit_present,
        ) {
            Some(RenderTiming {
                acquire_ms: elapsed_ms(render_start, after_acquire),
                tessellate_ms: elapsed_ms(after_acquire, after_tessellate),
                upload_ms: elapsed_ms(after_tessellate, after_upload),
                encode_ms: elapsed_ms(after_upload, after_encode),
                submit_present_ms: elapsed_ms(after_encode, after_submit_present),
                total_ms: elapsed_ms(render_start, after_submit_present),
            })
        } else {
            None
        };

        Ok(timing)
    }
}

fn tessellate(draw_list: &[DrawCmd], width: f32, height: f32) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    for cmd in draw_list {
        match cmd {
            DrawCmd::Rect { rect, color } => push_rect(&mut vertices, *rect, *color, width, height),
            DrawCmd::Text {
                pos,
                text,
                scale,
                color,
                clip_rect,
            } => push_text(&mut vertices, *pos, text, *scale, *color, *clip_rect, width, height),
        }
    }
    vertices
}

fn push_text(
    vertices: &mut Vec<Vertex>,
    pos: Vec2,
    text: &str,
    scale: f32,
    color: Color,
    clip_rect: Option<Rect>,
    width: f32,
    height: f32,
) {
    for glyph_rect in text::rasterize(text, pos, scale, color) {
        let Some(rect) = clip_rect
            .map(|clip| glyph_rect.rect.intersect(clip))
            .unwrap_or(Some(glyph_rect.rect))
        else {
            continue;
        };
        push_rect(vertices, rect, glyph_rect.color, width, height);
    }
}

fn push_rect(vertices: &mut Vec<Vertex>, rect: Rect, color: Color, width: f32, height: f32) {
    let min = to_ndc(rect.min, width, height);
    let max = to_ndc(rect.max, width, height);
    vertices.extend_from_slice(&[
        Vertex::new([min.x, min.y], color),
        Vertex::new([min.x, max.y], color),
        Vertex::new([max.x, min.y], color),
        Vertex::new([max.x, min.y], color),
        Vertex::new([min.x, max.y], color),
        Vertex::new([max.x, max.y], color),
    ]);
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
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
            ],
        }
    }
}

const SHADER: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;
