mod gui;
mod shaders;
mod vertex;

use backend::wgpu as wgpu;

#[derive(Clone, Copy)]
struct Config<'a> {
    #[allow(dead_code)]
    slices: u32,
    #[allow(dead_code)]
    stacks: u32,
    #[allow(dead_code)]
    globe_radius: f32,
    features: &'a [&'a str],
    shader_asset_path: &'a str,
    font_family: &'a str,
    font_size: f32,
    font_asset_path: &'a str,
}

impl<'a> backend::AppConfig for Config<'a> {
    fn window_title<'b>(self) -> &'b str {
        "ChronoMapper"
    }

    fn surface_format(self) -> backend::wgpu::TextureFormat {
        wgpu::TextureFormat::Rgba8Unorm
    }
}

impl<'a> Default for Config<'a> {
    fn default() -> Self {
        Self {
            slices: 100,
            stacks: 100,
            globe_radius: 1000.,
            features: &[
                "features/world_100.geojson",
                "features/world_200.geojson",
                "features/world_300.geojson",
                "features/world_400.geojson",
                "features/world_500.geojson",
                "features/world_600.geojson",
                "features/world_700.geojson",
                "features/world_800.geojson",
                "features/world_900.geojson",
            ],
            shader_asset_path: "shaders/render.wgsl",
            font_family: "Linux Biolinium G",
            font_size: 18.,
            font_asset_path: "fonts/biolinium.ttf",
        }
    }
}

struct App {
    context: gui::TextCtx,
    feature_selection_list: gui::SelectionList,
    pipeline: wgpu::RenderPipeline,
}

impl backend::App for App {
    type Config = Config<'static>;
    type RenderError = glyphon::RenderError;
    type UpdateError = anyhow::Error;

    async fn new(
        config: Self::Config, 
        data: backend::AppData<'_>,
    ) -> Result<Self, Self::UpdateError> where Self: Sized {
        use backend::AppConfig as _;

        use std::sync::Arc;

        let font_bytes = data.get_static_asset(config.font_asset_path)?.to_vec();
        let font_bytes = Arc::new(font_bytes);

        let mut context = gui::TextCtx::new(
            data.device, 
            data.queue, 
            config.surface_format(), 
            Arc::clone(&font_bytes), 
            config.font_family,
        );

        let feature_selection_list = gui::SelectionList::new(
            &mut context, 
            config.features.iter().copied(), 
            config.font_size,
        );

        let pipeline_layout = data.device.create_pipeline_layout(&{
            wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            }
        });

        let shader = data.device.create_shader_module({
            (shaders::load(&data, config.shader_asset_path).await)?
        });

        let pipeline = data.device.create_render_pipeline(&{
            wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vertex",
                    buffers: &[vertex::Vertex::layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fragment",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: config.surface_format(),
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
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
            }
        });

        Ok(Self { context, feature_selection_list, pipeline })
    }

    fn submit_passes(
        &mut self,
        encoder: &mut backend::wgpu::CommandEncoder,
        surface: &backend::wgpu::TextureView,
    ) -> Result<(), Self::RenderError> {
        let Self { 
            context, 
            feature_selection_list, 
            pipeline, .. 
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
        pass.set_pipeline(pipeline);

        feature_selection_list.render(context, &mut pass)?;

        Ok(())
    }

    fn handle_event(
        &mut self, 
        data: backend::AppData<'_>,
        event: backend::AppEvent,
    ) -> Result<bool, Self::UpdateError> {
        let Self { context, feature_selection_list, .. } = self;

        feature_selection_list.handle_event(context, data, event)
    }
}

backend::init!(App, Config => Config::default());