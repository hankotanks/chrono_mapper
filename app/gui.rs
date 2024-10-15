use backend::wgpu as wgpu;

pub struct TextCtx {
    font_system: glyphon::FontSystem,
    swash_cache: glyphon::SwashCache,
    atlas: glyphon::TextAtlas,
    renderer: glyphon::TextRenderer,
}

impl TextCtx {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        font_bytes: std::sync::Arc<Vec<u8>>,
        font_family: &'static str,
    ) -> Self {
        let mut font_system = glyphon::FontSystem::new_with_fonts({
            Some(glyphon::fontdb::Source::Binary(font_bytes))
        }); font_system.db_mut().set_fantasy_family(font_family);

        let mut atlas = glyphon::TextAtlas::new(
            device, 
            queue, 
            surface_format,
        );

        let renderer = glyphon::TextRenderer::new(
            &mut atlas, 
            device, 
            wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            }, None
        );

        Self {
            font_system,
            swash_cache: glyphon::SwashCache::new(),
            atlas,
            renderer,
        }
    }

    pub fn render<'p, 'a: 'p>(
        &'a self,
        #[allow(unused_variables)]
        pass: &mut wgpu::RenderPass<'p>,
    ) -> Result<(), glyphon::RenderError> {
        self.renderer.render(&self.atlas, pass)
    }
}

pub struct SelectionList {
    idx: usize,
    idx_prev: usize,
    scroll: usize,
    line_maxima: usize,
    visible: bool,
    width_maxima: f32,
    buffer: glyphon::Buffer,
}

impl SelectionList {
    const COLOR_FOCUS: glyphon::Color = glyphon::Color::rgb(255, 0, 0);
    const COLOR_BASE: glyphon::Color = glyphon::Color::rgb(255, 255, 255);
    const COLOR_LOADING: glyphon::Color = glyphon::Color::rgb(255, 150, 50);

    const SPACING: f32 = 4.;

    pub fn new<'a>(
        ctx: &mut TextCtx, 
        items: impl Iterator<Item = &'a str>, 
        font_size: f32,
    ) -> Self {
        let mut buffer = glyphon::Buffer::new(
            &mut ctx.font_system, 
            glyphon::Metrics::new(font_size, font_size + Self::SPACING),
        );

        let idx = 0;

        let font_attrs = glyphon::Attrs::new()
            .family(glyphon::Family::Fantasy)
            .color(Self::COLOR_BASE);

        buffer.set_rich_text(
            &mut ctx.font_system, 
            items.enumerate().map(|(idx_curr, item)| {
                let font_attrs = if idx_curr == idx {
                    font_attrs.color(Self::COLOR_FOCUS)
                } else { 
                    font_attrs 
                }; (item, font_attrs)
            }).flat_map(|line| [line, ("\n", font_attrs)]), 
            glyphon::Shaping::Basic,
        );

        let height = buffer.lines.len() as f32 * (font_size + Self::SPACING);

        buffer.set_size(&mut ctx.font_system, f32::MAX, height);

        buffer.shape_until_scroll(&mut ctx.font_system);

        let width_maxima = buffer
            .layout_runs()
            .fold(f32::MIN, |w, glyphon::LayoutRun { line_w, .. }| w.max(line_w));

        Self {
            idx, 
            idx_prev: 0, 
            scroll: 0,
            line_maxima: 0,
            visible: true,
            width_maxima,
            buffer,
        }
    }

    pub fn handle_event(
        &mut self,
        ctx: &mut TextCtx,
        data: backend::AppData<'_>,
        event: backend::AppEvent,
    ) -> anyhow::Result<bool> {
        let Self { 
            idx, 
            idx_prev,
            scroll, 
            line_maxima, 
            visible, 
            width_maxima,
            buffer, .. 
        } = self;

        match event {
            backend::AppEvent::Request(req) => {
                match req.state {
                    backend::RequestState::Fulfilled(_) =>  *idx_prev = *idx,
                    backend::RequestState::Failed => *idx = *idx_prev,
                    backend::RequestState::Loading => { /*  */ },
                }

                self.prepare(ctx, data)?;

                Ok(true)
            },
            backend::AppEvent::Resized(size) => {
                *line_maxima = (size.height as f32 / buffer.metrics().line_height).round() as usize;

                if buffer.lines.len() - *scroll < *line_maxima {
                    if let Some(idx_temp) = buffer.lines.len().checked_sub(*line_maxima) {
                        *scroll = idx_temp;
                    } else {
                        *scroll = 0;
                    }
                }

                buffer.set_size(&mut ctx.font_system, size.width as f32, buffer.size().1);

                self.prepare(ctx, data)?;

                // other components need to process size changes
                Ok(false)
            },
            backend::AppEvent::MouseScroll { 
                delta, 
                cursor: backend::Position { x, .. },
            } if *visible && x < *width_maxima => {
                if delta > 0. && (buffer.lines.len() - *scroll) > *line_maxima {
                    *scroll += 1;
                } else if delta < 0. && *scroll > 0 {
                    *scroll -= 1;
                }

                self.prepare(ctx, data)?;

                Ok(true)
            }
            backend::AppEvent::Mouse { 
                button: backend::event::MouseButton::Left, 
                state: backend::event::ElementState::Pressed, 
                cursor: backend::Position { x, y },
            } if *visible => {
                if let Some(glyphon::Cursor { line, .. }) = buffer.hit(x, y) {
                    match data.get(buffer.lines[line + *scroll].text()) {
                        Ok(_) => {
                            if *idx != *idx_prev {
                                // TODO: Don't repeat font_attrs all over this file
                                let font_attrs = glyphon::Attrs::new()
                                    .family(glyphon::Family::Fantasy)
                                    .color(Self::COLOR_BASE);

                                buffer.lines[*idx].set_attrs_list(glyphon::AttrsList::new(font_attrs));
                            }

                            *idx_prev = *idx; 
                            *idx = line + *scroll;

                            self.prepare(ctx, data)?;

                            Ok(true)
                        },
                        Err(e) => {
                            #[cfg(feature = "logging")]
                            backend::log::debug!("Failed to submit asset request.\n{e}");

                            Err(Into::<anyhow::Error>::into(e))
                        },
                    }
                } else { Ok(false) }
            },
            backend::AppEvent::Key { 
                code: backend::event::KeyCode::Tab, 
                state: backend::event::ElementState::Released,
            } => { 
                *visible = !(*visible);
                
                Ok(true) 
            },  _ => Ok(false),
        }
    }

    fn prepare(
        &mut self,
        ctx: &mut TextCtx,
        data: backend::AppData<'_>,
    ) -> Result<(), glyphon::PrepareError>  {
        match self.prepare_inner(ctx, data) {
            Ok(_) => Ok(()),
            Err(glyphon::PrepareError::AtlasFull) => {
                ctx.atlas.trim();

                match self.prepare_inner(ctx, data) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        #[cfg(feature = "logging")] 
                        backend::log::debug!("Failed to prepare layer selection pane.\n{e}");

                        Err(e)
                    },
                }
            }, #[allow(unreachable_patterns)] Err(e) => Err(e)
        }
    }

    fn prepare_inner(
        &mut self,
        ctx: &mut TextCtx,
        data: backend::AppData<'_>,
    ) -> Result<(), glyphon::PrepareError> {
        use glyphon::AttrsList;
        let Self { 
            idx, 
            idx_prev, 
            scroll,
            line_maxima,
            buffer, .. 
        } = self;

        let font_attrs = glyphon::Attrs::new()
            .family(glyphon::Family::Fantasy)
            .color(Self::COLOR_BASE);

        // update color of lines based on the state of the current asset request
        if *idx == *idx_prev {
            let font_attrs = font_attrs.color(Self::COLOR_FOCUS);
            buffer.lines[*idx].set_attrs_list(AttrsList::new(font_attrs));
        } else {
            buffer.lines[*idx_prev].set_attrs_list(AttrsList::new(font_attrs));

            let font_attrs = font_attrs.color(Self::COLOR_LOADING);
            buffer.lines[*idx].set_attrs_list(AttrsList::new(font_attrs));
        }

        // ensure all lines are updated
        buffer.shape_until_scroll(&mut ctx.font_system);

        // current size of the buffer
        let (width, height) = buffer.size();

        // this isn't the true screen resolution
        // instead a pixel-accurate height is provided for rendering
        let screen_resolution = glyphon::Resolution {
            width: width.round() as u32,
            height: (*line_maxima as f32 * buffer.metrics().line_height) as u32,
        };

        // offset in the opposite direction to achieve desired scroll effect
        let top = buffer.metrics().line_height * (*scroll as f32) * -1.;

        ctx.renderer.prepare(
            data.device, 
            data.queue, 
            &mut ctx.font_system, 
            &mut ctx.atlas, 
            screen_resolution,
            [
                glyphon::TextArea {
                    buffer,
                    left: 0.,
                    top,
                    scale: 1.,
                    bounds: glyphon::TextBounds {
                        left: 0,
                        top: top.floor() as i32,
                        right: width.round() as i32,
                        bottom: height.floor() as i32,
                    }, default_color: Self::COLOR_BASE,
                }
            ], &mut ctx.swash_cache,
        )
    }

    pub fn render<'p, 'a: 'p>(
        &'a self, 
        ctx: &'a TextCtx,
        #[allow(unused_variables)]
        pass: &mut wgpu::RenderPass<'p>,
    ) -> Result<(), glyphon::RenderError> {
        match self.visible {
            true => ctx.render(pass),
            false => Ok(()),
        }
    }
}