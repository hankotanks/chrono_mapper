mod geom;
mod feature_labels;
mod util;
mod camera;
mod map_tex;
mod loader;

use backend::wgpu as wgpu;

use std::{mem, sync};

#[derive(Clone, Copy)]
pub struct Config<'a> {
    pub surface_format: wgpu::TextureFormat,
    pub font_asset_path: &'a str,
    pub font_family: &'a str,
    pub slices: u32,
    pub stacks: u32,
    pub globe_radius: f32,
    pub globe_shader_asset_path: &'a str,
    pub basemap: &'a str,
    pub basemap_padding: backend::Size,
    pub features: &'a [backend::AssetRef<'a>],
    pub features_shader_asset_path: &'a str,
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
    #[allow(dead_code)] 
    texture: wgpu::Texture,
    texture_bind_group: wgpu::BindGroup,
    camera: camera::Camera,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    globe_radius: f32,
    globe: geom::Geometry<geom::GlobeVertex, ()>,
    globe_pipeline: wgpu::RenderPipeline,
    features: loader::FeatureManager,
    feature_geometry: geom::Geometry<geom::FeatureVertex, geom::FeatureMetadata>,
    feature_pipeline: wgpu::RenderPipeline,
    feature_labels: feature_labels::LabelEngine,
    screen_ray_density: u32,
    screen_rays: Vec<[f32; 3]>,
    screen_resolution: backend::Size,
}

impl backend::App for App {
    type Config = Config<'static>;
    type UpdateError = loader::LoaderError;
    type SubmissionError = glyphon::RenderError;

    async fn new(
        config: Self::Config, 
        device: &wgpu::Device, queue: &wgpu::Queue,
        assets: backend::Assets,
    ) -> anyhow::Result<Self> where Self: Sized {
        let basemap = map_tex::Basemap::from_bytes(
            backend::Assets::retrieve(config.basemap)?, 
            config.basemap_padding,
        )?;
        
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

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            }, &basemap.buffer,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * basemap.buffer_size.width),
                rows_per_image: Some(basemap.buffer_size.height),
            },
            wgpu::Extent3d {
                width: basemap.buffer_size.width,
                height: basemap.buffer_size.height,
                depth_or_array_layers: 1,
            },
        );

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
            (util::load_shader(config.globe_shader_asset_path).await)?
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
            (util::load_shader(config.features_shader_asset_path).await)?
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

        let feature_label_font_bytes = sync::Arc::new({
            backend::Assets::retrieve(config.font_asset_path)?.to_vec()
        });

        let feature_labels = feature_labels::LabelEngine::new(
            device,
            queue,
            config.surface_format,
            sync::Arc::clone(&feature_label_font_bytes),
            config.font_family,
        );

        let features = loader::FeatureManager::new(
            device,
            queue,
            config,
            sync::Arc::clone(&feature_label_font_bytes),
            assets,
        );

        Ok(Self {
            texture,
            texture_bind_group,
            camera: camera::Camera::new(config.globe_radius),
            camera_buffer,
            camera_bind_group,
            globe_radius: config.globe_radius,
            globe,
            globe_pipeline,
            features,
            feature_geometry: geom::Geometry::empty(device),
            feature_pipeline,
            feature_labels,
            screen_ray_density: config.feature_label_ray_density,
            screen_rays: Vec::with_capacity(0),
            screen_resolution: backend::Size::default(),
        })
    }

    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> Result<(), Self::SubmissionError> {       
        self.submit_globe_pass(encoder, surface);

        self.submit_feature_pass(encoder, surface)?;

        Ok(())
    }
    
    fn handle_event(
        &mut self,
        device: &wgpu::Device, queue: &wgpu::Queue,
        assets: backend::Assets,
        event: backend::AppEvent,
    ) -> bool {
        let Self {
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

        match event {
            event if features.handle_event(device, queue, event, assets) => { return true; }
            backend::AppEvent::Resized(size) => { *screen_resolution = size; },
            event if !camera.handle_event(event) => { return false; },
            _ => { /*  */ },
        }

        let camera_uniform = camera
            .update()
            .build_camera_uniform(*screen_resolution);

        // generate rays if its okay to prepare labels
        match (camera.movement_in_progress(), screen_rays.len()) {
            (true, screen_ray_count) if screen_ray_count > 0 => screen_rays.clear(), 
            (false, 0) => {
                let camera::CameraUniform {
                    view,
                    proj, ..
                } = camera_uniform;

                let backend::Size { width, height } = *screen_resolution;

                let gap = (width as f32 / *screen_ray_density as f32).ceil();

                for y in 0..(height as f32 / gap).ceil() as u32 {
                    let y = y as f32 / 5. - 1.;
                    for x in 0..*screen_ray_density {
                        let x = x as f32 / 5. - 1.;

                        let cursor = backend::Position { x, y };

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

            #[allow(unused_variables)]
            if let Err(e) = feature_labels.prepare(device, queue, *screen_resolution) {
                // clear screen rays to prevent rendering broken labels
                screen_rays.clear();

                #[cfg(feature = "logging")] 
                backend::log::debug!("Failed to position feature labels.\n{e}");
            }
        }

        queue.write_buffer(
            camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera_uniform]),
        );

        true
    }
    
    fn update(
        &mut self, 
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        #[allow(unused_variables)]
        asset_path: &str,
    ) -> Result<(), Self::UpdateError> {
        let Self {
            feature_geometry,
            feature_labels, 
            screen_rays,
            screen_resolution,
            camera,
            globe_radius, ..
        } = self;

        match self.features.load(device, queue, bytes, *screen_resolution) {
            Ok(repl) => {
                mem::replace(feature_geometry, repl).destroy();

                feature_labels.queue_labels_for_display(
                    &feature_geometry.metadata,
                    screen_rays,
                    camera.build_camera_uniform(*screen_resolution),
                    *globe_radius,
                );
    
                feature_labels
                    .prepare(device, queue, *screen_resolution)
                    .map_err(loader::LoaderError::LabelFailure)?;
            },
            #[allow(unused_variables)]
            Err(e) => {
                #[cfg(feature = "logging")] 
                backend::log::debug!("Failed to parse feature [{asset_path}].\n{e}");
            },
        }

        Ok(())
    }
}

impl App {
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
    ) -> Result<(), glyphon::RenderError> {
        let Self {
            camera_bind_group,
            features,
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

        features.render(&mut pass)?;

        Ok(())
    }
}