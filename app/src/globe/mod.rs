mod geom;
mod util;
mod camera;
mod map_tex;

use std::{mem, collections, io};

#[derive(Clone, Copy)]
pub struct GlobeConfig<'a> {
    pub format: wgpu::TextureFormat,
    pub slices: u32,
    pub stacks: u32,
    pub globe_radius: f32,
    pub globe_shader_asset_path: &'a str,
    pub basemap: &'a str,
    pub basemap_padding: winit::dpi::PhysicalSize<u32>,
    pub features: &'a [&'a str],
    pub features_shader_asset_path: &'a str,
}

impl backend::HarnessConfig for GlobeConfig<'static> {
    fn surface_format(self) -> wgpu::TextureFormat { self.format }
}

pub struct Globe {
    assets: collections::HashMap<&'static str, &'static [u8]>,
    basemap_data: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>>,
    texture: wgpu::Texture,
    texture_bind_group: wgpu::BindGroup,
    camera: camera::Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    globe: geom::Geometry<geom::GlobeVertex>,
    globe_pipeline: wgpu::RenderPipeline,
    features: FeatureManager,
    feature_geometry: geom::Geometry<geom::FeatureVertex>,
    feature_pipeline: wgpu::RenderPipeline,
}

impl backend::Harness for Globe {
    type Config = GlobeConfig<'static>;

    async fn new(
        config: Self::Config, 
        device: &wgpu::Device,
        #[allow(unused_variables)]
        assets: collections::HashMap<&'static str, &'static [u8]>,
    ) -> anyhow::Result<Self> where Self: Sized {
        let Self::Config { 
            format, 
            slices,
            stacks,
            globe_radius,
            globe_shader_asset_path,
            basemap,
            basemap_padding,
            features: _,
            features_shader_asset_path,
        } = config;

        let bytes = assets
            .get(basemap.replace("::", "/").as_str())
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

        let basemap = map_tex::Basemap::from_bytes(bytes, basemap_padding)?;
        
        let texture = device.create_texture(&{
            wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: basemap.buffer_size.width,
                    height: basemap.buffer_size.height,
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
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
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
            size: mem::size_of::<camera::CameraUniform>() as u64,
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

        let globe = geom::Geometry::build_globe_geometry(
            device, 
            slices, 
            stacks,
            globe_radius,
        );

        let globe_pipeline_layout = device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            }
        });

        let globe_pipeline_shader = device.create_shader_module({
            util::load_shader(&assets, globe_shader_asset_path)?
        });

        let globe_pipeline = device.create_render_pipeline(&{
            wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&globe_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &globe_pipeline_shader,
                    entry_point: "vertex",
                    buffers: &[geom::GlobeVertex::layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &globe_pipeline_shader,
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

        let feature_pipeline_layout = device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            }
        });

        let feature_pipeline_shader = device.create_shader_module({
            util::load_shader(&assets, features_shader_asset_path)?
        });

        let feature_pipeline = device.create_render_pipeline(&{
            wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&feature_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &feature_pipeline_shader,
                    entry_point: "vertex",
                    buffers: &[geom::FeatureVertex::layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &feature_pipeline_shader,
                    entry_point: "fragment",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
            }
        }); 

        Ok(Self {
            assets,
            basemap_data: Some(basemap.buffer),
            texture,
            texture_bind_group,
            camera: camera::Camera::new(globe_radius),
            camera_buffer,
            camera_bind_group,
            globe,
            globe_pipeline,
            features: FeatureManager::from(config),
            feature_geometry: geom::Geometry::empty(device),
            feature_pipeline,
        })
    }

    fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Self {
            assets,
            basemap_data,
            texture,
            camera,
            camera_buffer,
            features, 
            feature_geometry, ..
        } = self;

        queue.write_buffer(
            camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera.update().build_camera_uniform()])
        );

        if let Some(basemap_data) = basemap_data.take() {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                }, &basemap_data,
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

        if let Some(new_feature_geometry) = features.load_if_ready(device, assets) {
            mem::replace(feature_geometry, new_feature_geometry).destroy();
        }
    }
    
    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()> {       
        self.submit_globe_pass(encoder, surface);

        self.submit_feature_pass(encoder, surface);

        Ok(())
    }
    
    fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool {
        use winit::keyboard::{PhysicalKey, KeyCode};

        match event {
            winit::event::DeviceEvent::Key(winit::event::RawKeyEvent { 
                physical_key: PhysicalKey::Code(KeyCode::Space), 
                state: winit::event::ElementState::Released,
            }) => {
                self.features.queue(); true
            },
            _ => self.camera.handle_event(event),
        }
        
    }
    
    fn handle_resize(
        &mut self,
        size: winit::dpi::PhysicalSize<u32>,
        scale: f32, 
    ) { self.camera.handle_resize(size, scale); }
}

impl Globe {
    fn submit_globe_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) {
        let Self {
            texture_bind_group,
            camera_bind_group,
            globe: geom::Geometry {
                vertex_buffer,
                indices,
                index_buffer, ..
            },
            globe_pipeline, ..
        } = self;

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: surface,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0., g: 0., b: 0., a: 1. }),
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
        pass.set_pipeline(globe_pipeline);

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
        pass.set_bind_group(1, texture_bind_group, &[]);

        // draw
        pass.draw_indexed(0..(indices.len() as u32), 0, 0..1);
    }

    fn submit_feature_pass(
        &self, 
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) {
        let Self {
            camera_bind_group,
            feature_geometry: geom::Geometry {
                vertex_buffer,
                indices,
                index_buffer, ..
            },
            feature_pipeline, ..
        } = self;

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: surface,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        pass.set_pipeline(feature_pipeline);

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
        pass.draw_indexed(0..(indices.len() as u32), 0, 0..1);
    }
}

struct FeatureManager {
    idx: usize,
    features: &'static [&'static str],
    queued: bool,
    slices: u32,
    stacks: u32,
    globe_radius: f32,
}

impl From<GlobeConfig<'static>> for FeatureManager {
    fn from(value: GlobeConfig<'static>) -> Self {
        let GlobeConfig {
            slices,
            stacks,
            globe_radius,
            features, ..
        } = value;
        
        Self {
            idx: 0,
            features,
            queued: true,
            slices,
            stacks,
            globe_radius,
        }
    }
}

impl FeatureManager {
    fn queue(&mut self) { 
        self.queued = true; 
    }

    fn load_if_ready(
        &mut self,
        device: &wgpu::Device,
        assets: &collections::HashMap<&str, &[u8]>,
    ) -> Option<geom::Geometry<geom::FeatureVertex>> {
        if !self.queued { 
            return None; 
        }

        let feature = self.features[self.idx];

        let result = util::load_features_from_geojson(assets, feature)
            .and_then(|features| {
                geom::Geometry::build_feature_geometry_earcut(
                    device, 
                    features.as_slice(),
                    self.slices, 
                    self.stacks,
                    self.globe_radius, 
                ).map_err(anyhow::Error::from)
            });

        self.idx += 1;
        self.idx %= self.features.len();

        match result {
            Ok(geometry) => {
                self.queued = false; Some(geometry)
            }, Err(_) => None,
        }
    }
}