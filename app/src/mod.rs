mod geom;
mod feature_labels;
mod util;
mod camera;
mod map_tex;

use std::{mem, io};

#[derive(Clone, Copy)]
pub struct Config<'a> {
    pub surface_format: wgpu::TextureFormat,
    pub font_asset_path: backend::AssetRef<'a>,
    pub font_family: &'a str,
    pub slices: u32,
    pub stacks: u32,
    pub globe_radius: f32,
    pub globe_shader_asset_path: backend::AssetRef<'a>,
    pub basemap: backend::AssetRef<'a>,
    pub basemap_padding: winit::dpi::PhysicalSize<u32>,
    pub features: &'a [backend::AssetRef<'a>],
    pub features_shader_asset_path: backend::AssetRef<'a>,
    // the number of rays to distribute across the screen's width
    // vertical ray density is proportional to the window's aspect ratio
    pub feature_label_ray_density: u32,
}

impl backend::AppConfig for Config<'static> {
    fn surface_format(self) -> wgpu::TextureFormat { 
        self.surface_format 
    }
}

pub struct App {
    basemap_data: Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>>,
    texture: wgpu::Texture,
    texture_bind_group: wgpu::BindGroup,
    camera: camera::Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    globe_radius: f32,
    globe: geom::Geometry<geom::GlobeVertex, ()>,
    globe_pipeline: wgpu::RenderPipeline,
    features: FeatureManager,
    feature_geometry: geom::Geometry<geom::FeatureVertex, geom::FeatureMetadata>,
    feature_pipeline: wgpu::RenderPipeline,
    feature_labels: feature_labels::LabelEngine,
    screen_ray_density: u32,
    screen_rays: Vec<[f32; 3]>,
    screen_resolution: winit::dpi::PhysicalSize<u32>,
}

impl backend::App for App {
    type Config = Config<'static>;

    async fn new(
        config: Self::Config, 
        device: &wgpu::Device, queue: &wgpu::Queue,
    ) -> anyhow::Result<Self> where Self: Sized {
        let bytes = backend::Assets::retrieve(config.basemap)
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

        let basemap = map_tex::Basemap::from_bytes(bytes, config.basemap_padding)?;
        
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
                format: config.surface_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[config.surface_format],
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
            config.slices, 
            config.stacks,
            config.globe_radius,
        );

        let globe_pipeline_layout = device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            }
        });

        let globe_pipeline_shader = device.create_shader_module({
            util::load_shader(config.globe_shader_asset_path)?
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
                            format: config.surface_format,
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
            util::load_shader(config.features_shader_asset_path)?
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
                            format: config.surface_format,
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

        let font_bytes = backend::Assets::retrieve(config.font_asset_path)
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?
            .to_vec();

        let feature_labels = feature_labels::LabelEngine::new(
            device,
            queue,
            config.surface_format,
            font_bytes,
            config.font_family,
        );

        Ok(Self {
            basemap_data: Some(basemap.buffer),
            texture,
            texture_bind_group,
            camera: camera::Camera::new(config.globe_radius),
            camera_buffer,
            camera_bind_group,
            globe_radius: config.globe_radius,
            globe,
            globe_pipeline,
            features: FeatureManager::from(config),
            feature_geometry: geom::Geometry::empty(device),
            feature_pipeline,
            feature_labels,
            screen_ray_density: config.feature_label_ray_density,
            screen_rays: Vec::with_capacity(0),
            screen_resolution: winit::dpi::PhysicalSize::default(),
        })
    }

    fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> anyhow::Result<()> {
        let Self {
            basemap_data,
            texture,
            camera,
            camera_buffer,
            globe_radius,
            features, 
            feature_geometry, 
            feature_labels,
            screen_ray_density, 
            screen_rays,
            screen_resolution, ..
        } = self;

        let camera_uniform = camera
            .update()
            .build_camera_uniform();

        // generate rays if its okay to prepare labels
        match (camera.movement_in_progress(), screen_rays.len()) {
            (true, screen_ray_count) if screen_ray_count > 0 => screen_rays.clear(), 
            (false, 0) => {
                let camera::CameraUniform {
                    view,
                    proj, ..
                } = camera_uniform;
    
                let winit::dpi::PhysicalSize { width, height } = *screen_resolution;
    
                let gap = (width as f32 / *screen_ray_density as f32).ceil();
    
                for y in 0..(height as f32 / gap).ceil() as u32 {
                    let y = y as f32 / 5. - 1.;
                    for x in 0..*screen_ray_density {
                        let x = x as f32 / 5. - 1.;
    
                        let cursor = winit::dpi::PhysicalPosition { x, y };
    
                        screen_rays.push(util::cursor_to_world_ray(view, proj, cursor));
                    }
                }
            }, _ => { /*  */ },
        }

        if !camera.movement_in_progress() {
            feature_labels.queue_labels_for_display(
                &feature_geometry.metadata,
                screen_rays,
                camera_uniform,
                *globe_radius,
            );

            feature_labels.prepare(device, queue, *screen_resolution)?;
        }

        queue.write_buffer(
            camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera_uniform])
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

        if let Some(repl) = features.load_if_ready(device) {
            mem::replace(feature_geometry, repl).destroy();

            feature_labels.queue_labels_for_display(
                &feature_geometry.metadata,
                screen_rays,
                camera_uniform,
                *globe_radius,
            );

            feature_labels.prepare(device, queue, *screen_resolution)?;
        }

        Ok(())
    }
    
    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()> {       
        self.submit_globe_pass(encoder, surface)?;

        self.submit_feature_pass(encoder, surface)?;

        Ok(())
    }
    
    fn handle_event(
        &mut self,
        event: winit::event::DeviceEvent,
    ) -> bool {
        use winit::keyboard::{PhysicalKey, KeyCode};

        match event {
            winit::event::DeviceEvent::Key(winit::event::RawKeyEvent { 
                physical_key: PhysicalKey::Code(KeyCode::Space), 
                state: winit::event::ElementState::Released,
            }) => {
                self.features.queue();

                true 
            },
            _ => self.camera.handle_event(event),
        }  
    }
    
    fn handle_resize(
        &mut self,
        size: winit::dpi::PhysicalSize<u32>,
        scale: f32, 
    ) {
        self.camera.handle_resize(size, scale);

        self.screen_resolution = size;
    }
}

impl App {
    fn submit_globe_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
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

        Ok(())
    }

    fn submit_feature_pass(
        &self, 
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()> {
        let Self {
            camera_bind_group,
            feature_geometry: geom::Geometry {
                vertex_buffer,
                indices,
                index_buffer, ..
            },
            feature_pipeline, 
            feature_labels, 
            screen_rays, ..
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

        // only render labels if screen rays are generated
        // if they aren't then the camera is being moved
        if !screen_rays.is_empty() {
            feature_labels.render(&mut pass)?;
        }

        Ok(())
    }
}

struct FeatureManager {
    idx: usize,
    features: &'static [backend::AssetRef<'static>],
    queued: bool,
    slices: u32,
    stacks: u32,
    globe_radius: f32,
}

impl From<Config<'static>> for FeatureManager {
    fn from(value: Config<'static>) -> Self {
        let Config {
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
        &mut self, device: &wgpu::Device,
    ) -> Option<geom::Geometry<geom::FeatureVertex, geom::FeatureMetadata>> {
        if !self.queued { 
            return None; 
        }

        let feature = self.features[self.idx];

        let result = util::load_features_from_geojson(feature)
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