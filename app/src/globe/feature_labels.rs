use std::{iter, sync};

pub struct Label<'a> {
    pub text: &'a str,
    pub pos: [f32; 2],
    pub color: [u8; 3],
    pub feature_area: f32,
}

struct LabelBuffer {
    buffer: glyphon::Buffer,
    bounds: glyphon::TextBounds,
    color: glyphon::Color,
    feature_area: f32,
}

impl LabelBuffer {
    fn as_text_area(&self) -> glyphon::TextArea {
        let Self { 
            buffer, 
            bounds: glyphon::TextBounds {
                left,
                top,
                right,
                bottom,
            }, 
            color, .. 
        } = self;

        glyphon::TextArea {
            buffer,
            left: *left as f32,
            top: *top as f32,
            scale: 1.,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: *right,
                bottom: *bottom,
            },
            default_color: *color,
        }
    }
}

pub struct LabelEngine {
    font_system: glyphon::FontSystem,
    font_attrs: glyphon::Attrs<'static>,
    swash_cache: glyphon::SwashCache,
    atlas: glyphon::TextAtlas,
    buffers: Vec<LabelBuffer>,
    renderer: glyphon::TextRenderer,
}

impl LabelEngine {
    const METRICS: glyphon::Metrics = glyphon::Metrics::new(18., 18.);

    pub fn new(
        device: &wgpu::Device, 
        queue: &wgpu::Queue, 
        surface_format: wgpu::TextureFormat,
        font_bytes: Vec<u8>,
        font_family: &'static str,
    ) -> Self {
        let font_system = glyphon::FontSystem::new_with_fonts({
            use glyphon::fontdb::Source;

            iter::once(Source::Binary(sync::Arc::new(font_bytes)))
        });

        let swash_cache = glyphon::SwashCache::new();

        let mut atlas = glyphon::TextAtlas::new(
            device, 
            queue, 
            surface_format
        );

        let renderer = glyphon::TextRenderer::new(
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
            font_attrs: glyphon::Attrs::new().family(glyphon::Family::Name(font_family)),
            swash_cache,
            atlas,
            buffers: Vec::new(),
            renderer,
        }
    }

    pub fn queue_labels_for_display<'a>(
        &mut self, 
        labels: impl Iterator<Item = Label<'a>>,
        screen_resolution: winit::dpi::PhysicalSize<u32>,
    ) {
        let Self { 
            font_system, 
            font_attrs, 
            buffers, .. 
        } = self;

        buffers.clear();

        let winit::dpi::PhysicalSize { width, height } = screen_resolution;
        
        for Label { text, pos, color, feature_area } in labels {
            let pos = [
                (pos[0] + 1.) * 0.5 * width as f32,
                (pos[1] * -1. + 1.) * 0.5 * height as f32,
            ];

            let mut buffer = glyphon::Buffer::new(font_system, Self::METRICS);

            buffer.set_size(
                font_system, 
                width as f32- pos[0], 
                height as f32 - pos[1],
            );

            buffer.set_text(
                font_system, 
                text, 
                *font_attrs, 
                glyphon::Shaping::Basic,
            );

            buffer.set_wrap(font_system, glyphon::Wrap::Word);

            buffer.shape_until_scroll(font_system);

            let buffer_width: f32 = buffer
                .layout_runs()
                .fold(0., |bw, run| bw.max(run.line_w));

            let left = pos[0].floor() as i32;

            let right = if buffer_width == 0. {
                width as i32
            } else {
                left + buffer_width.ceil() as i32
            };

            let top = pos[1].floor() as i32;

            let bottom = top + buffer.lines.len() as i32 * //
                Self::METRICS.line_height.ceil() as i32;

            let bounds = glyphon::TextBounds { left, top, right, bottom };

            let [r, g, b] = color;

            let diff = 255u8 - r.max(g).max(b);

            buffers.push(LabelBuffer {
                buffer,
                bounds,
                color: glyphon::Color::rgb(r + diff, g + diff, b + diff),
                feature_area,
            });
        }

        { // cull overlapping buffers
            use std::collections::{HashMap, HashSet};

            let mut intrs_tests = HashMap::new();
            for i in 0..buffers.len() {
                let mut others: HashSet<usize> = HashSet::from_iter(0..buffers.len());
                others.remove(&i);

                intrs_tests.insert(i, others);
            }

            for i in 0..buffers.len() {
                if !intrs_tests.contains_key(&i) { continue; }

                let fst = &buffers[i];

                let js: Vec<usize> = intrs_tests[&i].iter().copied().collect();
                for j in js {
                    if !intrs_tests.contains_key(&j) { continue; }

                    let snd = &buffers[j];

                    if overlapping_text_bounds(fst.bounds, snd.bounds) {
                        if fst.feature_area > snd.feature_area {
                            intrs_tests.remove(&j);
                        } else {
                            intrs_tests.remove(&i);

                            break;
                        }
                    }
                }
            }

            for i in (0..buffers.len()).rev() {
                if !intrs_tests.contains_key(&i) {
                    buffers.remove(i);
                }
            }
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_resolution: winit::dpi::PhysicalSize<u32>,
    ) -> Result<(), glyphon::PrepareError> {
        let Self { 
            font_system, 
            swash_cache,
            atlas,
            renderer, 
            buffers, .. 
        } = self;

        if buffers.is_empty() { return Ok(()); }

        let winit::dpi::PhysicalSize { width, height } = screen_resolution;

        renderer.prepare(
            device, 
            queue, 
            font_system, 
            atlas, 
            glyphon::Resolution { width, height }, 
            buffers.iter().map(LabelBuffer::as_text_area), 
            swash_cache,
        )?;

        buffers.clear();

        Ok(())
    }

    pub fn render<'p, 'a: 'p>(
        &'a self, 
        pass: &mut wgpu::RenderPass<'p>,
    ) -> Result<(), glyphon::RenderError> {
        let Self { atlas, renderer, .. } = self;

        renderer.render(atlas, pass)
    }
}

fn overlapping_text_bounds(a: glyphon::TextBounds, b: glyphon::TextBounds) -> bool {
    !(
        b.left > a.right || //
        b.right < a.left || //
        b.top > a.bottom || //
        b.bottom < a.top
    )
}