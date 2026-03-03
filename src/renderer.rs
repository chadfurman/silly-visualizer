use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AudioUniforms {
    pub time: f32,
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
    pub beat: f32,
    pub seed: f32,
    pub _pad: f32,
    pub resolution: [f32; 2],
    pub bands: [f32; 16],
}

impl Default for AudioUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            bass: 0.0,
            mids: 0.0,
            highs: 0.0,
            energy: 0.0,
            beat: 0.0,
            seed: 0.0,
            _pad: 0.0,
            resolution: [0.0, 0.0],
            bands: [0.0; 16],
        }
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_groups: [wgpu::BindGroup; 2],
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
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
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
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(prev_frame_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Self {
        pollster::block_on(Self::init(window))
    }

    async fn init(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("failed to create device");

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_config = surface
            .get_default_config(&adapter, width, height)
            .expect("surface is not supported by the adapter");
        surface.configure(&device, &surface_config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("visualizer shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/visualizer.wgsl").into(),
            ),
        });

        // Create uniform buffer initialized with zeroed data
        let uniforms = AudioUniforms::default();
        let uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("audio uniforms buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST,
            });

        // Create feedback textures for ping-pong rendering
        let tex_a = create_feedback_texture(
            &device,
            width,
            height,
            surface_config.format,
            "feedback texture A",
        );
        let tex_b = create_feedback_texture(
            &device,
            width,
            height,
            surface_config.format,
            "feedback texture B",
        );
        let view_a = tex_a.create_view(&wgpu::TextureViewDescriptor::default());
        let view_b = tex_b.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler for previous frame sampling
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("feedback sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Bind group layout: uniform buffer + prev frame texture + sampler
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("feedback bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
            });

        // Two bind groups for ping-pong: each uses the OTHER texture as prev_frame
        // bind_groups[0]: renders to tex_a, reads from tex_b (prev)
        // bind_groups[1]: renders to tex_b, reads from tex_a (prev)
        let bind_group_0 = create_bind_group(
            &device,
            &bind_group_layout,
            &uniform_buffer,
            &view_b,
            &sampler,
            "bind group 0 (prev=B)",
        );
        let bind_group_1 = create_bind_group(
            &device,
            &bind_group_layout,
            &uniform_buffer,
            &view_a,
            &sampler,
            "bind group 1 (prev=A)",
        );

        // Pipeline layout now includes our bind group layout
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                immediate_size: 0,
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        Self {
            device,
            queue,
            surface,
            surface_config,
            render_pipeline,
            uniform_buffer,
            bind_group_layout,
            bind_groups: [bind_group_0, bind_group_1],
            feedback_textures: [tex_a, tex_b],
            feedback_views: [view_a, view_b],
            sampler,
            frame_index: 0,
            start_time: Instant::now(),
            seed: 0.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);

            // Recreate feedback textures at new size
            let tex_a = create_feedback_texture(
                &self.device,
                width,
                height,
                self.surface_config.format,
                "feedback texture A",
            );
            let tex_b = create_feedback_texture(
                &self.device,
                width,
                height,
                self.surface_config.format,
                "feedback texture B",
            );
            let view_a =
                tex_a.create_view(&wgpu::TextureViewDescriptor::default());
            let view_b =
                tex_b.create_view(&wgpu::TextureViewDescriptor::default());

            // Recreate bind groups with new texture views
            self.bind_groups = [
                create_bind_group(
                    &self.device,
                    &self.bind_group_layout,
                    &self.uniform_buffer,
                    &view_b,
                    &self.sampler,
                    "bind group 0 (prev=B)",
                ),
                create_bind_group(
                    &self.device,
                    &self.bind_group_layout,
                    &self.uniform_buffer,
                    &view_a,
                    &self.sampler,
                    "bind group 1 (prev=A)",
                ),
            ];

            self.feedback_textures = [tex_a, tex_b];
            self.feedback_views = [view_a, view_b];
            self.frame_index = 0;
        }
    }

    pub fn update_uniforms(&self, uniforms: &AudioUniforms) {
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[*uniforms]),
        );
    }

    pub fn randomize_seed(&mut self) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        self.seed = (nanos as f32) / 1_000_000_000.0;
        log::info!("randomized seed: {:.4}", self.seed);
    }

    pub fn render(&mut self, uniforms: &mut AudioUniforms) {
        // Fill in time and resolution
        uniforms.time = self.start_time.elapsed().as_secs_f32();
        uniforms.seed = self.seed;
        uniforms.resolution = [
            self.surface_config.width as f32,
            self.surface_config.height as f32,
        ];

        // Write uniforms to GPU
        self.update_uniforms(uniforms);

        let output = match self.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost) => {
                self.surface
                    .configure(&self.device, &self.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("out of GPU memory");
                return;
            }
            Err(e) => {
                log::warn!("surface error: {e:?}");
                return;
            }
        };

        // Determine which texture to render to (curr) and which is previous
        // frame_index=0: render to tex_a, read prev from tex_b -> use bind_groups[0]
        // frame_index=1: render to tex_b, read prev from tex_a -> use bind_groups[1]
        let curr_idx = self.frame_index;
        let curr_view = &self.feedback_views[curr_idx];

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        // Pass 1: Render scene to offscreen feedback texture (with prev frame as input)
        {
            let mut render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("offscreen render pass"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: curr_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        },
                    )],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_groups[curr_idx], &[]);
            render_pass.draw(0..3, 0..1);
        }

        // Copy offscreen texture to surface for display
        let surface_texture = &output.texture;
        encoder.copy_texture_to_texture(
            self.feedback_textures[curr_idx].as_image_copy(),
            surface_texture.as_image_copy(),
            wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Swap ping-pong index for next frame
        self.frame_index = 1 - self.frame_index;
    }
}
