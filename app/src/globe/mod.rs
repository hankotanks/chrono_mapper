mod vertex;
mod util;
mod camera;

use std::io;

#[derive(Clone, Copy)]
pub struct GlobeConfig {
    format: wgpu::TextureFormat,
    slices: u32,
    stacks: u32,
    globe_radius: f32,
    shader_asset_path: &'static str,
    basemap: &'static str,
    basemap_borders: winit::dpi::PhysicalSize<u32>,
}

// TODO: GlobeConfig should not implement Default
// because members like `shader_asset_path` are out of scope
impl Default for GlobeConfig {
    fn default() -> Self {
        Self { 
            format: wgpu::TextureFormat::Rgba8Unorm,
            slices: 100,
            stacks: 100,
            globe_radius: 1000.,
            shader_asset_path: "shaders::render",
            basemap: "blue_marble_2048.tif", // https://visibleearth.nasa.gov/images/57752/blue-marble-land-surface-shallow-water-and-shaded-topography
            basemap_borders: winit::dpi::PhysicalSize::default(),
        }
    }
}

impl backend::HarnessConfig for GlobeConfig {
    fn surface_format(self) -> wgpu::TextureFormat { self.format }
}

pub struct Globe {
    geometry: vertex::Geometry,
    basemap_data: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>>,
    texture: wgpu::Texture,
    texture_bind_group: wgpu::BindGroup,
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
            globe_radius,
            shader_asset_path,
            basemap,
            basemap_borders,
        } = config;

        let bytes = assets
            .get(basemap.replace("::", "/").as_str())
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

        let basemap_data = image::load_from_memory(bytes)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?
            .to_rgba8();

        let basemap_data: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = {
            use image::GenericImageView as _;

            let winit::dpi::PhysicalSize { width, height } = basemap_borders;

            basemap_data.view(
                width, 
                height, 
                basemap_data.width() - width * 2, 
                basemap_data.height() - height * 2,
            ).to_image()
        };

        let texture = device.create_texture(&{
            wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: basemap_data.width(),
                    height: basemap_data.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[format],
            }
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest, ..Default::default()
        });

        let texture_bind_group_layout = device.create_bind_group_layout(&{
            wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    }
                ],
            }
        });

        let texture_bind_group = device.create_bind_group(&{
            wgpu::BindGroupDescriptor {
                label: None,
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&{
                            texture.create_view(&wgpu::TextureViewDescriptor::default())
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&texture_sampler),
                    }
                ],
            }
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<camera::CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&{
            wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            }
        });

        let camera_bind_group = device.create_bind_group(&{
            wgpu::BindGroupDescriptor {
                label: None,
                layout: &camera_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            }
        });

        let pipeline_layout = device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            }
        });

        let pipeline_shader = device.create_shader_module({
            util::load_shader(&assets, shader_asset_path)?
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

        // generate the globe for later
        let geometry = vertex::Geometry::generate(
            device, 
            slices, 
            stacks,
            globe_radius,
        );

        Ok(Self {
            geometry,
            basemap_data: Some(basemap_data),
            texture,
            texture_bind_group,
            camera: camera::Camera::new(globe_radius * 1.5, 1.),
            camera_buffer,
            camera_bind_group,
            pipeline,
        })
    }

    fn update(&mut self, queue: &wgpu::Queue) {
        let Self {
            basemap_data,
            texture,
            camera,
            camera_buffer, ..
        } = self;

        queue.write_buffer(
            camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera.update().build_camera_uniform()])
        );

        if let Some(basemap_data) = basemap_data.take() {
            queue.write_texture(
                // Tells wgpu where to copy the pixel data
                wgpu::ImageCopyTexture {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                // The actual pixel data
                &basemap_data,
                // The layout of the texture
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * basemap_data.width()),
                    rows_per_image: Some(basemap_data.height()),
                },
                wgpu::Extent3d {
                    width: basemap_data.width(),
                    height: basemap_data.height(),
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    
    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
        let Self {
            geometry: vertex::Geometry {
                vertex_buffer,
                index_count,
                index_buffer, ..
            },
            pipeline, 
            texture_bind_group: tex_bind_group,
            camera_bind_group, ..
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

        // bind mercator texture
        pass.set_bind_group(1, tex_bind_group, &[]);

        // draw
        pass.draw_indexed(0..*index_count, 0, 0..1);

        Ok(())
    }
    
    fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool {
        self.camera.handle_event(event)
    }
    
    fn handle_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.camera.handle_resize(size);
    }
}