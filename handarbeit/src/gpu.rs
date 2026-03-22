use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::geom::{Color, Point, Rect, Size, to_ndc};
use crate::text;

const ATLAS_SIZE: u32 = 1024;

pub enum DrawCmd {
    Rect {
        rect: Rect,
        color: Color,
    },
    Text {
        pos: Point,
        layout: text::TextLayout,
        color: Color,
        clip_rect: Option<Rect>,
    },
}

pub struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    rect_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    text_sampler: wgpu::Sampler,
    atlas_pages: Vec<AtlasPage>,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyph>,
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

        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect shader"),
            source: wgpu::ShaderSource::Wgsl(RECT_SHADER.into()),
        });
        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[SolidVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
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

        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("text bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let text_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text atlas sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
        });
        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text pipeline layout"),
            bind_group_layouts: &[&text_bind_group_layout],
            immediate_size: 0,
        });
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                buffers: &[TextInstance::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
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
            window,
            surface,
            device,
            queue,
            config,
            rect_pipeline,
            text_pipeline,
            text_bind_group_layout,
            text_sampler,
            atlas_pages: Vec::new(),
            glyph_cache: HashMap::new(),
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

    // This is the main render function
    pub fn render(&mut self, draw_list: &[DrawCmd]) -> Result<(), wgpu::SurfaceError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(());
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let (rect_vertices, text_batches) = self.tessellate(
            draw_list,
            self.config.width as f32,
            self.config.height as f32,
        );

        // TODO: reuse vertex and text buffers
        let rect_vertex_buffer = (!rect_vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("rect vertices"),
                    contents: bytemuck::cast_slice(&rect_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });

        // TODO: reuse vertex and text buffers
        let mut text_buffers = Vec::new();
        for (page_index, instances) in text_batches.into_iter().enumerate() {
            if instances.is_empty() {
                continue;
            }

            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("text instances"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            text_buffers.push((page_index, buffer, instances.len() as u32));
        }

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
                multiview_mask: None,
            });

            if let Some(rect_vertex_buffer) = &rect_vertex_buffer {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(0, rect_vertex_buffer.slice(..));
                pass.draw(0..rect_vertices.len() as u32, 0..1);
            }

            if !text_buffers.is_empty() {
                pass.set_pipeline(&self.text_pipeline);
                for (page_index, buffer, instance_count) in &text_buffers {
                    pass.set_bind_group(0, &self.atlas_pages[*page_index].bind_group, &[]);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(0..6, 0..*instance_count);
                }
            }
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        output.present();

        Ok(())
    }

    fn tessellate(
        &mut self,
        draw_list: &[DrawCmd],
        width: f32,
        height: f32,
    ) -> (Vec<SolidVertex>, Vec<Vec<TextInstance>>) {
        let mut rect_vertices = Vec::new();
        let mut text_batches: Vec<Vec<TextInstance>> = Vec::new();

        for cmd in draw_list {
            match cmd {
                DrawCmd::Rect { rect, color } => {
                    push_rect(&mut rect_vertices, *rect, *color, width, height);
                }
                DrawCmd::Text {
                    pos,
                    layout,
                    color,
                    clip_rect,
                } => self.push_text(
                    &mut text_batches,
                    *pos,
                    layout,
                    *color,
                    *clip_rect,
                    width,
                    height,
                ),
            }
        }

        (rect_vertices, text_batches)
    }

    fn push_text(
        &mut self,
        text_batches: &mut Vec<Vec<TextInstance>>,
        pos: Point,
        layout: &text::TextLayout,
        color: Color,
        clip_rect: Option<Rect>,
        width: f32,
        height: f32,
    ) {
        if layout.glyphs.is_empty() {
            return;
        }

        let baseline = pos.y + layout.pixel_height as f32;

        for glyph in &layout.glyphs {
            // We ask for the result of the glyph rasterization in a cache.
            // TODO: we currently only have font size as special keying value
            // we will later use font face and font style also. Note that color
            // is not important here bc it's greyscale rasterization.
            // cached_glyph is very important since it uploads it's glyphs to the
            // atlas of the GPU
            let cached = self.cached_glyph(GlyphCacheKey {
                glyph_id: glyph.glyph_id,
                pixel_height: layout.pixel_height,
            });

            let Some(page_index) = cached.page_index else {
                continue;
            };

            // INFO: we do one quad per glyph here. Then we clip the rects to
            // the most fitting ones.
            let rect = Rect::from_origin_and_size(
                Point::new(
                    pos.x + glyph.x + cached.left,
                    baseline - glyph.y_offset - cached.top,
                ),
                Size::new(cached.width, cached.height),
            );

            let Some((clipped_rect, uv_min, uv_max)) =
                clip_textured_rect(rect, cached.uv_min, cached.uv_max, clip_rect)
            else {
                continue;
            };

            while text_batches.len() <= page_index {
                // TODO: maybe slab allocate text batches
                text_batches.push(Vec::new());
            }

            let min = to_ndc(clipped_rect.min, width, height);
            let max = to_ndc(clipped_rect.max, width, height);

            text_batches[page_index].push(TextInstance {
                min: [min.x, min.y],
                max: [max.x, max.y],
                uv_min,
                uv_max,
                color,
            });
        }
    }

    // TODO: make glyph cache key more complex by using font weight and font
    // styles and font faces.
    fn cached_glyph(&mut self, key: GlyphCacheKey) -> CachedGlyph {
        if let Some(cached) = self.glyph_cache.get(&key).copied() {
            return cached;
        }

        // here FreeType does it's work. Don't worry, we cache the results per glyph.
        let bitmap = text::rasterize_glyph(key.glyph_id, key.pixel_height);
        let mut cached = CachedGlyph {
            page_index: None,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            left: bitmap.left,
            top: bitmap.top,
            width: bitmap.width as f32,
            height: bitmap.height as f32,
        };

        if bitmap.width > 0 && bitmap.height > 0 {
            let (page_index, uv_min, uv_max) = self.upload_glyph_bitmap(&bitmap);
            cached.page_index = Some(page_index);
            cached.uv_min = uv_min;
            cached.uv_max = uv_max;
        }

        self.glyph_cache.insert(key, cached);
        cached
    }

    // Here we cache the glyph on the GPU in atlas pages. We cache these overarching over frames
    // so that we can reuse them.
    // TODO: review simple allocation algorithm here.
    fn upload_glyph_bitmap(&mut self, bitmap: &text::GlyphBitmap) -> (usize, [f32; 2], [f32; 2]) {
        let (page_index, origin) = self.allocate_atlas_region(bitmap.width, bitmap.height);
        let page = &self.atlas_pages[page_index];

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin[0],
                    y: origin[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width),
                rows_per_image: Some(bitmap.height),
            },
            wgpu::Extent3d {
                width: bitmap.width,
                height: bitmap.height,
                depth_or_array_layers: 1,
            },
        );

        let uv_min = [
            origin[0] as f32 / ATLAS_SIZE as f32,
            origin[1] as f32 / ATLAS_SIZE as f32,
        ];
        let uv_max = [
            (origin[0] + bitmap.width) as f32 / ATLAS_SIZE as f32,
            (origin[1] + bitmap.height) as f32 / ATLAS_SIZE as f32,
        ];

        (page_index, uv_min, uv_max)
    }

    fn allocate_atlas_region(&mut self, width: u32, height: u32) -> (usize, [u32; 2]) {
        for (index, page) in self.atlas_pages.iter_mut().enumerate() {
            if let Some(origin) = page.allocate(width, height) {
                return (index, origin);
            }
        }

        let mut page = AtlasPage::new(
            &self.device,
            &self.text_bind_group_layout,
            &self.text_sampler,
        );
        let origin = page
            .allocate(width, height)
            .expect("glyph should fit in empty atlas page");
        self.atlas_pages.push(page);
        (self.atlas_pages.len() - 1, origin)
    }
}

fn push_rect(vertices: &mut Vec<SolidVertex>, rect: Rect, color: Color, width: f32, height: f32) {
    let min = to_ndc(rect.min, width, height);
    let max = to_ndc(rect.max, width, height);

    vertices.extend_from_slice(&[
        SolidVertex::new([min.x, min.y], color),
        SolidVertex::new([min.x, max.y], color),
        SolidVertex::new([max.x, min.y], color),
        SolidVertex::new([max.x, min.y], color),
        SolidVertex::new([min.x, max.y], color),
        SolidVertex::new([max.x, max.y], color),
    ]);
}

fn clip_textured_rect(
    rect: Rect,
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    clip_rect: Option<Rect>,
) -> Option<(Rect, [f32; 2], [f32; 2])> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }

    let clipped_rect = clip_rect
        .map(|clip| rect.intersection(&clip))
        .unwrap_or(Some(rect))?;

    let u_span = uv_max[0] - uv_min[0];
    let v_span = uv_max[1] - uv_min[1];
    let left_ratio = (clipped_rect.min.x - rect.min.x) / rect.width();
    let right_ratio = (clipped_rect.max.x - rect.min.x) / rect.width();
    let top_ratio = (clipped_rect.min.y - rect.min.y) / rect.height();
    let bottom_ratio = (clipped_rect.max.y - rect.min.y) / rect.height();

    Some((
        clipped_rect,
        [
            uv_min[0] + u_span * left_ratio,
            uv_min[1] + v_span * top_ratio,
        ],
        [
            uv_min[0] + u_span * right_ratio,
            uv_min[1] + v_span * bottom_ratio,
        ],
    ))
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct GlyphCacheKey {
    glyph_id: u32,
    pixel_height: u32,
}

#[derive(Clone, Copy)]
struct CachedGlyph {
    // Page in the atlas
    page_index: Option<usize>,
    // X, Y coordinates into the atlas
    uv_min: [f32; 2],
    uv_max: [f32; 2],

    // Left and top is where FreeType usually finds the glyph to start in its position
    // (position given by harfbuzz) after rasterizing.
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

struct AtlasPage {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl AtlasPage {
    fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph atlas bind group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        Self {
            texture,
            bind_group,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<[u32; 2]> {
        if width > ATLAS_SIZE || height > ATLAS_SIZE {
            return None;
        }

        if self.cursor_x + width > ATLAS_SIZE {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }

        if self.cursor_y + height > ATLAS_SIZE {
            return None;
        }

        let origin = [self.cursor_x, self.cursor_y];
        self.cursor_x += width;
        self.row_height = self.row_height.max(height);
        Some(origin)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SolidVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl SolidVertex {
    fn new(position: [f32; 2], color: Color) -> Self {
        Self { position, color }
    }

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SolidVertex>() as u64,
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TextInstance {
    min: [f32; 2],
    max: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    color: [f32; 4],
}

impl TextInstance {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32x4
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}

const RECT_SHADER: &str = r#"
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

const TEXT_SHADER: &str = r#"
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) min: vec2<f32>,
    @location(1) max: vec2<f32>,
    @location(2) uv_min: vec2<f32>,
    @location(3) uv_max: vec2<f32>,
    @location(4) color: vec4<f32>,
) -> VsOut {
    let positions = array<vec2<f32>, 6>(
        vec2<f32>(min.x, min.y),
        vec2<f32>(min.x, max.y),
        vec2<f32>(max.x, min.y),
        vec2<f32>(max.x, min.y),
        vec2<f32>(min.x, max.y),
        vec2<f32>(max.x, max.y),
    );
    let uvs = array<vec2<f32>, 6>(
        vec2<f32>(uv_min.x, uv_min.y),
        vec2<f32>(uv_min.x, uv_max.y),
        vec2<f32>(uv_max.x, uv_min.y),
        vec2<f32>(uv_max.x, uv_min.y),
        vec2<f32>(uv_min.x, uv_max.y),
        vec2<f32>(uv_max.x, uv_max.y),
    );

    var out: VsOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let alpha = textureSample(atlas, atlas_sampler, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;
