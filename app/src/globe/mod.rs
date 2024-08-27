mod shaders;
mod vertex;
mod camera;

use std::{mem, sync};

use winit::{dpi, event};

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
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
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
        #[allow(unused_variables)]
        assets: std::collections::HashMap<&'a str, &'a [u8]>,
        #[allow(unused_variables)]
        window: sync::Arc<winit::window::Window>,
    ) -> anyhow::Result<Self> where Self: Sized {
        let Self::Config { 
            format, 
            slices,
            stacks,
            shader_asset_path,
        } = config;

        fn create_surface_target<'a>(
            #[allow(unused_variables)] window: sync::Arc<winit::window::Window>,
        ) -> anyhow::Result<wgpu::SurfaceTarget<'a>> {
            #[cfg(target_arch="wasm32")] {
                use wasm_bindgen::JsCast as _;

                let window = web_sys::window().expect("no global `window` exists");
                let document = window.document().expect("should have a document on window");
                let canvas = document
                    .get_element_by_id("screen")
                    .unwrap();

                let handle = canvas.dyn_into::<web_sys::HtmlCanvasElement>()
                    .expect("element must be a canvas");

                Ok(wgpu::SurfaceTarget::Canvas(handle))
            }
            
            #[cfg(not(target_arch = "wasm32"))] {
                Ok(wgpu::SurfaceTarget::Window(Box::new(window)))
            }
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::default()
        });

        let surface = instance.create_surface({
            create_surface_target(sync::Arc::clone(&window))?
        })?;

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let device_desc = wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
        };

        let (device, queue) = adapter
            .request_device(&device_desc, None)
            .await
            .unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);

        let wgpu::SurfaceCapabilities {
            present_modes,
            alpha_modes, ..
        } = surface_capabilities;

        let dpi::PhysicalSize {
            width,
            height,
        } = window.inner_size().max(dpi::PhysicalSize::new(1, 1));

        // Construct the surface configuration
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: present_modes[0],
            alpha_mode: alpha_modes[0],
            view_formats: vec![format],
            desired_maximum_frame_latency: 1,
        };

        // Configure the surface (no longer platform-specific)
        surface.configure(&device, &surface_config);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<camera::CameraUniform>() as u64,
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
                    entry_point: "vertex", // TODO
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

        let geometry = vertex::Geometry::generate(slices, stacks, &device);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            geometry,
            camera: camera::Camera::new(5., 1.), // TODO: `aspect` must change on resize
            camera_buffer,
            camera_bind_group,
            pipeline,
        })
    }

    fn resize(&mut self, size: dpi::PhysicalSize<u32>) {
        let Self {
            device,
            surface,
            surface_config, .. 
        } = self;

        surface_config.width = size.width;
        surface_config.height = size.height;

        surface.configure(device, surface_config);
    }
    
    fn update(&mut self) -> anyhow::Result<()> {
        let Self {
            device,
            queue,
            surface, 
            pipeline, 
            camera,
            camera_buffer,
            camera_bind_group,
            geometry: vertex::Geometry {
                vertex_buffer,
                index_count,
                index_buffer, ..
            }, ..
        } = self;

        queue.write_buffer(
            camera_buffer, 
            0, 
            bytemuck::cast_slice(&[camera.update().build_camera_uniform()])
        );

        let output = surface.get_current_texture()?;

        let view = output.texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder({
            &wgpu::CommandEncoderDescriptor::default()
        });

        {
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0., g: 0., b: 1., a: 0. }),
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
                wgpu::IndexFormat::Uint16,
            );

            // set vertex buffer
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));

            // bind camera
            pass.set_bind_group(0, camera_bind_group, &[]);

            // draw
            pass.draw_indexed(0..*index_count, 0, 0..1);
        }

        // Submit for execution (async)
        queue.submit(Some(encoder.finish()));

        // Schedule for drawing
        output.present();

        Ok(())
    }
    
    fn handle_event(&mut self, event: event::DeviceEvent) -> bool {
        self.camera.handle_event(event)
    }
}