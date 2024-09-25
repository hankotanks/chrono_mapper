use std::{iter, sync};

use super::{camera, geom, util};

pub struct Label {
    pub text: String,
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
    visible_feature_labels: Vec<Label>,
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
            visible_feature_labels: Vec::with_capacity(0),
            renderer,
        }
    }

    pub fn queue_labels_for_display(
        &mut self, 
        metadata: &geom::FeatureMetadata,
        rays: &[[f32; 3]],
        camera_uniform: camera::CameraUniform,
        globe_radius: f32,
    ) {
        use core::f32;

        let Self { visible_feature_labels, .. } = self;

        let geom::FeatureMetadata {
            entries,
            colors,
            bounding_boxes, .. 
        } = metadata;

        let camera::CameraUniform {
            eye,
            view,
            proj,
        } = camera_uniform;

        let maxima_sq = util::hemisphere_maxima_sq(eye, globe_radius);

        for ray in rays.iter().copied() {
            for (bb, idx) in bounding_boxes.iter().copied() {
                let geom::BoundingBox { centroid, tl, tr, bl, br } = bb;

                let a = util::intrs(eye, ray, tl, tr, bl, maxima_sq);
                let b = util::intrs(eye, ray, tr, br, bl, maxima_sq);
            
                if a < f32::MAX || b < f32::MAX {
                    if let Some(serde_json::Value::String(name)) = entries[idx].get("NAME") {
                        let pos = util::world_to_screen_space(centroid, view, proj);

                        let bb_minima = util::world_to_screen_space(tl, view, proj);
                        let bb_maxima = util::world_to_screen_space(br, view, proj);

                        let bb_width = (bb_maxima[0] - bb_minima[0]).abs() * 0.5;
                        let bb_height = (bb_maxima[1] - bb_minima[1]).abs() * 0.5;

                        let label = Label {
                            text: name.to_owned(),
                            pos,
                            color: colors[idx],
                            feature_area: bb_width * bb_height,
                        };

                        visible_feature_labels.push(label);
                    }
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
            font_attrs,
            swash_cache,
            atlas,
            renderer, 
            visible_feature_labels, .. 
        } = self;

        let mut buffers = Vec::with_capacity(visible_feature_labels.len());

        let winit::dpi::PhysicalSize { width, height } = screen_resolution;
        
        for Label { text, pos, color, feature_area } in visible_feature_labels.drain(0..) {
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

            #[allow(unused_parens)]
            buffer.set_text(
                font_system, 
                text.as_str(), 
                (*font_attrs), 
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

            buffers.push(LabelBuffer {
                buffer,
                bounds,
                color: glyphon::Color::rgb(r, g, b),
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

                    let overlapping = !(
                        snd.bounds.left > fst.bounds.right || //
                        snd.bounds.right < fst.bounds.left || //
                        snd.bounds.top > fst.bounds.bottom || //
                        snd.bounds.bottom < fst.bounds.top
                    );

                    if overlapping {
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

        if buffers.is_empty() { return Ok(()); }

        renderer.prepare(
            device, 
            queue, 
            font_system, 
            atlas, 
            glyphon::Resolution { width, height }, 
            buffers.iter().map(LabelBuffer::as_text_area), 
            swash_cache,
        )?;

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