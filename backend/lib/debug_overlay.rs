use std::{iter, sync};

pub struct DebugOverlay {
    font_system: glyphon::FontSystem,
    swash_cache: glyphon::SwashCache,
    atlas: glyphon::TextAtlas,
    text_buffer: glyphon::Buffer,
    text: String,
    text_renderer: glyphon::TextRenderer,
}

impl DebugOverlay {
    pub fn new(
        device: &wgpu::Device, 
        queue: &wgpu::Queue, 
        surface_format: wgpu::TextureFormat,
        overlay_font_bytes: Vec<u8>,
    ) -> Self {
        let mut font_system = glyphon::FontSystem::new_with_fonts({
            use glyphon::fontdb::Source;

            iter::once(Source::Binary(sync::Arc::new(overlay_font_bytes)))
        });

        let swash_cache = glyphon::SwashCache::new();

        let mut atlas = glyphon::TextAtlas::new(
            device, 
            queue, 
            surface_format
        );

        let mut text_buffer = glyphon::Buffer::new(
            &mut font_system, 
            glyphon::Metrics::new(30., 40.),
        );

        text_buffer.set_wrap(
            &mut font_system,
            glyphon::Wrap::Glyph,
        );

        let text_renderer = glyphon::TextRenderer::new(
            &mut atlas, 
            device, 
            wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            None
        );

        Self {
            font_system,
            swash_cache,
            atlas,
            text_buffer,
            text: String::from(""),
            text_renderer,
        }
    }

    pub fn update_overlay_text(&mut self, repl: Option<&str>) -> bool {
        let Self {
            font_system,
            text,
            text_buffer, ..
        } = self;

        // if the message hasn't changed, return early
        if text.is_empty() && repl.is_none() || Some(text.as_str()) == repl {
            return false;
        }

        text.clear();

        if let Some(repl) = repl {
            text.push_str(repl);
        }

        text_buffer.set_text(
            font_system,
            text.as_str(),
            glyphon::Attrs::new().family(glyphon::Family::Name("Linux Biolinium G")),
            glyphon::Shaping::Basic,
        );

        true
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window_size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<(), glyphon::PrepareError> {
        const PADDING: f32 = 10.;

        let Self {
            font_system,
            atlas,
            swash_cache,
            text_buffer,
            text_renderer, ..
        } = self;

        let winit::dpi::PhysicalSize { width, height } = window_size;

        text_buffer.set_size(
            font_system,
            width as f32 - PADDING * 2.,
            height as f32 - PADDING * 2.,
        );
        
        let text_areas = glyphon::TextArea {
            buffer: text_buffer,
            left: PADDING,
            top: PADDING,
            scale: 1.0,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: (width as f32 - PADDING * 2.).ceil() as i32,
                bottom: (height as f32 - PADDING * 2.).ceil() as i32,
            },
            default_color: glyphon::Color::rgb(255, 0, 0),
        };

        text_renderer.prepare(
            device,
            queue,
            font_system,
            atlas,
            glyphon::Resolution { width, height },
            iter::once(text_areas),
            swash_cache,
        )
    }

    pub fn render(
        &self, 
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> Result<(), glyphon::RenderError> {
        let Self { atlas, text_renderer, .. } = self;

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
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        text_renderer.render(atlas, &mut pass)
    }
}