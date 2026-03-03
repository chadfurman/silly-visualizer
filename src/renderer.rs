use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use wgpu::util::DeviceExt;
use winit::window::Window;

pub use crate::uniforms::{AudioUniforms, SceneUniforms};

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    scene_uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_groups: [wgpu::BindGroup; 2],
    scene_bind_group: wgpu::BindGroup,
    feedback_textures: [wgpu::Texture; 2],
    feedback_views: [wgpu::TextureView; 2],
    sampler: wgpu::Sampler,
    frame_index: usize,
    start_time: Instant,
    seed: f32,
}

fn create_feedback_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    prev_frame_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(prev_frame_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn create_uniform_buffer(device: &wgpu::Device, contents: &[u8], label: &str) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("feedback sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}

fn create_scene_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene bind group"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
    })
}

struct GpuResources {
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    scene_buf: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_groups: [wgpu::BindGroup; 2],
    scene_bg: wgpu::BindGroup,
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    sampler: wgpu::Sampler,
}

fn create_gpu_resources(ctx: &GpuContext) -> GpuResources {
    let (w, h) = (ctx.surface_config.width, ctx.surface_config.height);
    let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("visualizer shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/visualizer.wgsl").into()),
    });
    let uniform_buf = create_uniform_buffer(&ctx.device, bytemuck::cast_slice(&[AudioUniforms::default()]), "audio uniforms buffer");
    let scene_buf = create_uniform_buffer(&ctx.device, bytemuck::cast_slice(&[SceneUniforms::default()]), "scene uniforms buffer");
    let (ta, tb, va, vb) = create_texture_pair(&ctx.device, w, h, ctx.surface_config.format);
    let sampler = create_sampler(&ctx.device);
    let layout = create_audio_bind_group_layout(&ctx.device);
    let scene_layout = create_scene_bind_group_layout(&ctx.device);
    let scene_bg = create_scene_bind_group(&ctx.device, &scene_layout, &scene_buf);
    let bind_groups = create_ping_pong_bind_groups(&ctx.device, &layout, &uniform_buf, &va, &vb, &sampler);
    let pipeline = create_render_pipeline(&ctx.device, &shader, &layout, &scene_layout, ctx.surface_config.format);
    GpuResources { pipeline, uniform_buf, scene_buf, layout, bind_groups, scene_bg, textures: [ta, tb], views: [va, vb], sampler }
}

struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

async fn request_adapter(instance: &wgpu::Instance, surface: &wgpu::Surface<'_>) -> wgpu::Adapter {
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("failed to find a suitable GPU adapter")
}

async fn create_gpu_context(window: Arc<Window>) -> GpuContext {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY, ..Default::default()
    });
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = request_adapter(&instance, &surface).await;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default()).await
        .expect("failed to create device");
    let size = window.inner_size();
    let (w, h) = (size.width.max(1), size.height.max(1));
    let mut surface_config = surface.get_default_config(&adapter, w, h)
        .expect("surface is not supported by the adapter");
    surface_config.usage |= wgpu::TextureUsages::COPY_DST;
    surface.configure(&device, &surface_config);
    GpuContext { device, queue, surface, surface_config }
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Self {
        pollster::block_on(Self::init(window))
    }

    async fn init(window: Arc<Window>) -> Self {
        let ctx = create_gpu_context(window).await;
        let resources = create_gpu_resources(&ctx);
        Self {
            device: ctx.device, queue: ctx.queue, surface: ctx.surface, surface_config: ctx.surface_config,
            render_pipeline: resources.pipeline, uniform_buffer: resources.uniform_buf,
            scene_uniform_buffer: resources.scene_buf, bind_group_layout: resources.layout,
            bind_groups: resources.bind_groups, scene_bind_group: resources.scene_bg,
            feedback_textures: resources.textures, feedback_views: resources.views,
            sampler: resources.sampler, frame_index: 0, start_time: Instant::now(), seed: 0.0,
        }
    }

    pub fn surface_size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.recreate_feedback_textures(width, height);
    }

    fn recreate_feedback_textures(&mut self, w: u32, h: u32) {
        let (tex_a, tex_b, view_a, view_b) = create_texture_pair(&self.device, w, h, self.surface_config.format);
        self.bind_groups = create_ping_pong_bind_groups(
            &self.device, &self.bind_group_layout, &self.uniform_buffer, &view_a, &view_b, &self.sampler,
        );
        self.feedback_textures = [tex_a, tex_b];
        self.feedback_views = [view_a, view_b];
        self.frame_index = 0;
    }

    pub fn update_uniforms(&self, uniforms: &AudioUniforms) {
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
    }

    pub fn update_scene_uniforms(&self, uniforms: &SceneUniforms) {
        self.queue.write_buffer(&self.scene_uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
    }

    pub fn randomize_seed(&mut self) {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
        self.seed = (nanos as f32) / 1_000_000_000.0;
        log::info!("randomized seed: {:.4}", self.seed);
    }

    pub fn render(&mut self, uniforms: &mut AudioUniforms) {
        self.prepare_uniforms(uniforms);
        self.update_uniforms(uniforms);
        let Some(output) = self.acquire_surface() else { return };
        let curr_idx = self.frame_index;
        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("render encoder") },
        );
        self.encode_render_pass(&mut encoder, curr_idx);
        self.copy_to_surface(&mut encoder, &output.texture, curr_idx);
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        self.frame_index = 1 - self.frame_index;
    }

    fn prepare_uniforms(&self, uniforms: &mut AudioUniforms) {
        uniforms.time = self.start_time.elapsed().as_secs_f32();
        uniforms.seed = self.seed;
        uniforms.resolution = [self.surface_config.width as f32, self.surface_config.height as f32];
    }

    fn acquire_surface(&self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            Ok(tex) => Some(tex),
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.surface_config);
                None
            }
            Err(wgpu::SurfaceError::OutOfMemory) => { log::error!("out of GPU memory"); None }
            Err(e) => { log::warn!("surface error: {e:?}"); None }
        }
    }

    #[allow(clippy::too_many_lines)] // wgpu descriptor verbosity
    fn encode_render_pass(&self, encoder: &mut wgpu::CommandEncoder, idx: usize) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("offscreen render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.feedback_views[idx],
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.render_pipeline);
        pass.set_bind_group(0, &self.bind_groups[idx], &[]);
        pass.set_bind_group(1, &self.scene_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn copy_to_surface(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture, idx: usize) {
        encoder.copy_texture_to_texture(
            self.feedback_textures[idx].as_image_copy(),
            target.as_image_copy(),
            wgpu::Extent3d { width: self.surface_config.width, height: self.surface_config.height, depth_or_array_layers: 1 },
        );
    }
}

fn create_texture_pair(
    device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::Texture, wgpu::TextureView, wgpu::TextureView) {
    let a = create_feedback_texture(device, w, h, format, "feedback texture A");
    let b = create_feedback_texture(device, w, h, format, "feedback texture B");
    let va = a.create_view(&wgpu::TextureViewDescriptor::default());
    let vb = b.create_view(&wgpu::TextureViewDescriptor::default());
    (a, b, va, vb)
}

fn create_ping_pong_bind_groups(
    device: &wgpu::Device, layout: &wgpu::BindGroupLayout, buf: &wgpu::Buffer,
    view_a: &wgpu::TextureView, view_b: &wgpu::TextureView, sampler: &wgpu::Sampler,
) -> [wgpu::BindGroup; 2] {
    [
        create_bind_group(device, layout, buf, view_b, sampler, "bind group 0 (prev=B)"),
        create_bind_group(device, layout, buf, view_a, sampler, "bind group 1 (prev=A)"),
    ]
}

#[allow(clippy::too_many_lines)] // wgpu descriptor verbosity
fn create_audio_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("feedback bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_scene_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scene bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }],
    })
}

#[allow(clippy::too_many_lines)] // wgpu descriptor verbosity
fn create_render_pipeline(
    device: &wgpu::Device, shader: &wgpu::ShaderModule,
    bind_group_layout: &wgpu::BindGroupLayout, scene_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline layout"),
        bind_group_layouts: &[bind_group_layout, scene_layout],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState { module: shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: shader, entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList, strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw, cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false, polygon_mode: wgpu::PolygonMode::Fill, conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        multiview_mask: None,
        cache: None,
    })
}
