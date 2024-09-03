mod shaders;
mod vertex;
mod camera;

#[derive(Clone, Copy)]
pub struct GlobeConfig {
    format: wgpu::TextureFormat,
    slices: u32,
    stacks: u32,
    shader_asset_path: &'static str,
}

// TODO: GlobeConfig should not implement Default
// because members like `shader_asset_path` are out of scope
impl Default for GlobeConfig {
    fn default() -> Self {
        Self { 
            format: wgpu::TextureFormat::Rgba8Unorm,
            slices: 20,
            stacks: 20,
            shader_asset_path: "shaders::render",
        }
    }
}

pub struct Globe {
    geometry: vertex::Geometry,
    camera: camera::Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl backend::Harness for Globe {
    type Config = GlobeConfig;

    async fn new<'a>(
        config: Self::Config, 
        device: &wgpu::Device,
        #[allow(unused_variables)]
        assets: std::collections::HashMap<&'a str, &'a [u8]>,
    ) -> anyhow::Result<Self> where Self: Sized {
        let Self::Config { 
            format, 
            slices,
            stacks,
            shader_asset_path,
        } = config;

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<camera::CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&{
            wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }
                ],
            }
        });

        let camera_bind_group = device.create_bind_group(&{
            wgpu::BindGroupDescriptor {
                label: None,
                layout: &camera_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    }
                ],
            }
        });

        let pipeline_layout = device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            }
        });

        let pipeline_shader = device.create_shader_module({
            shaders::load_shader(shader_asset_path, &assets)?
        });

        let pipeline = device.create_render_pipeline(&{
            wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &pipeline_shader,
                    entry_point: "vertex",
                    buffers: &[vertex::Vertex::layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &pipeline_shader,
                    entry_point: "fragment",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })
                    ],
                }),
                depth_stencil: None,
                multiview: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
            }
        });

        let geometry = vertex::Geometry::generate(slices, stacks, device);

        Ok(Self {
            geometry,
            camera: camera::Camera::new(5., 1.),
            camera_buffer,
            camera_bind_group,
            pipeline,
        })
    }

    fn update(&mut self, queue: &wgpu::Queue) {
        let Self {
            camera,
            camera_buffer, ..
        } = self;

        queue.write_buffer(
            camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera.update().build_camera_uniform()])
        );
    }
    
    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
        let Self {
            pipeline, 
            camera_bind_group,
            geometry: vertex::Geometry {
                vertex_buffer,
                index_count,
                index_buffer, ..
            }, ..
        } = self;

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: surface,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0., g: 0., b: 1., a: 1. }),
                store: wgpu::StoreOp::Store,
            },
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        // bind render pipeline
        pass.set_pipeline(pipeline);

        // set index buffer
        pass.set_index_buffer(
            index_buffer.slice(..), 
            wgpu::IndexFormat::Uint32,
        );

        // set vertex buffer
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));

        // bind camera
        pass.set_bind_group(0, camera_bind_group, &[]);

        // draw
        pass.draw_indexed(0..*index_count, 0, 0..1);

        Ok(())
    }
    
    fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool {
        self.camera.handle_event(event)
    }
}