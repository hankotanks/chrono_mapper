mod shaders;
mod vertex;

use winit::dpi;

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
    pipeline: wgpu::RenderPipeline,
}

impl backend::Harness for Globe {
    type Config = GlobeConfig;

    async fn new<'a>(
        config: Self::Config, 
        #[allow(unused_variables)]
        assets: std::collections::HashMap<&'a str, &'a [u8]>,
        window: &winit::window::Window,
    ) -> anyhow::Result<Self> where Self: Sized {
        let Self::Config { 
            format, 
            slices,
            stacks,
            shader_asset_path,
        } = config;

        fn create_surface_target(
            window: &winit::window::Window
        ) -> anyhow::Result<wgpu::SurfaceTargetUnsafe> {
            #[cfg(target_arch="wasm32")] {
                use wgpu::rwh;
        
                let target = wgpu::SurfaceTargetUnsafe::RawHandle { 
                    raw_display_handle: rwh::RawDisplayHandle::Web({
                        rwh::WebDisplayHandle::new()
                    }),
                    raw_window_handle: rwh::RawWindowHandle::Web({
                        // NOTE: This id is hard-coded
                        rwh::WebWindowHandle::new(2024)
                    }),
                };

                Ok(target)
            }
            
            #[cfg(not(target_arch = "wasm32"))] unsafe {
                Ok(wgpu::SurfaceTargetUnsafe::from_window(&window)?)
            }
        }

        let instance = wgpu::Instance::default();

        let surface = unsafe {
            let target = create_surface_target(window)?;

            instance.create_surface_unsafe(target)?
        };

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

        let pipeline_layout = device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
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
                    front_face: wgpu::FrontFace::Ccw,
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
            pipeline,
        })
    }

    fn resize(&mut self, size: dpi::PhysicalSize<u32>) {
        let Self {
            device,
            surface,
            surface_config, .. 
        } = self;

        log::info!("resized to {:?}", size); // TODO

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
            geometry: vertex::Geometry {
                vertex_buffer,
                index_count,
                index_buffer, ..
            }, ..
        } = self;

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

            // bind render pipeline
            pass.set_pipeline(pipeline);

            // set index buffer
            pass.set_index_buffer(
                index_buffer.slice(..), 
                wgpu::IndexFormat::Uint16,
            );

            // set vertex buffer
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));

            // draw
            pass.draw_indexed(0..*index_count, 0, 0..1);
        }

        // Submit for execution (async)
        queue.submit(Some(encoder.finish()));

        // Schedule for drawing
        output.present();

        Ok(())
    }
}