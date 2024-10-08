use backend::wgpu as wgpu;

use super::geom;

use std::{str, fmt, error};

#[derive(Debug)]
pub enum LoaderError {
    InvalidPath(str::Utf8Error),
    InvalidGeoJson(geojson::Error),
    BrokenGeometry(earcutr::Error),
    LabelFailure(glyphon::PrepareError),
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoaderError::InvalidPath(err) => write!(f, "{}", err),
            LoaderError::InvalidGeoJson(err) => write!(f, "{}", err),
            LoaderError::BrokenGeometry(err) => write!(f, "{}", err),
            LoaderError::LabelFailure(err) => write!(f, "{}", err),
        }
    }
}

impl error::Error for LoaderError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> { None }
}

pub struct FeatureManager {
    idx: usize,
    toggled: bool,
    feature_paths: &'static [backend::AssetRef<'static>],
    slices: u32,
    stacks: u32,
    globe_radius: f32,
    font_system: glyphon::FontSystem,
    font_attrs: glyphon::Attrs<'static>,
    swash_cache: glyphon::SwashCache,
    atlas: glyphon::TextAtlas,
    buttons: glyphon::Buffer,
    renderer: glyphon::TextRenderer,
}

impl FeatureManager {
    const METRICS: glyphon::Metrics = glyphon::Metrics::new(24., 28.);

    const COLOR_FOCUS: glyphon::Color = glyphon::Color::rgb(255, 0, 0);
    const COLOR_BASIC: glyphon::Color = glyphon::Color::rgb(255, 255, 255);

    pub fn new(
        device: &wgpu::Device, 
        queue: &wgpu::Queue,
        config: crate::Config<'static>,
        font_bytes: std::sync::Arc<Vec<u8>>,
        assets: backend::Assets,
    ) -> Self {
        let mut font_system = glyphon::FontSystem::new_with_fonts({
            use glyphon::fontdb::Source;

            Some(Source::Binary(font_bytes))
        });

        let font_attrs = glyphon::Attrs::new()
            .family(glyphon::Family::Name(config.font_family));

        let mut atlas = glyphon::TextAtlas::new(
            device, 
            queue, 
            config.surface_format
        );

        let buttons = glyphon::Buffer::new(&mut font_system, Self::METRICS);

        let renderer = glyphon::TextRenderer::new(
            &mut atlas, 
            device, 
            wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            }, None
        );

        if assets.request(config.features[0]).is_err() {
            #[cfg(feature = "logging")]
            backend::log::debug!("load interrupted");
        }

        Self {
            idx: 0,
            toggled: true,
            feature_paths: config.features,
            slices: config.slices,
            stacks: config.stacks,
            globe_radius: config.globe_radius,
            font_system,
            font_attrs,
            swash_cache: glyphon::SwashCache::new(),
            atlas,
            buttons,
            renderer,
        }
    }

    pub fn toggle_visibility(&mut self) {
        let Self { toggled, .. } = self;

        *toggled = !(*toggled);
    }

    pub fn request(
        &mut self,
        assets: &backend::Assets,
        #[allow(unused_variables)]
        backend::Position { x, y }: backend::Position,
    ) -> bool {
        let Self {
            idx,
            toggled, 
            feature_paths, 
            buttons, ..
        } = self;

        if !(*toggled) { return false; }

        let temp = (y / Self::METRICS.line_height).floor() as usize;
        match buttons.layout_runs().nth(temp) {
            Some(glyphon::LayoutRun { line_w, .. }) if x < line_w.ceil() => {
                *idx = temp;

                if assets.request(feature_paths[*idx]).is_err() {
                    #[cfg(feature = "logging")]
                    backend::log::debug!("load interrupted");
                }

                true
            }, _ => false,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device, 
        queue: &wgpu::Queue,
        backend::Size { width, height }: backend::Size,
    ) -> Result<(), glyphon::PrepareError> {
        let Self {
            idx,
            feature_paths,
            buttons,
            font_system, 
            font_attrs, 
            atlas,
            swash_cache,
            renderer, ..
        } = self;

        let spans = feature_paths
            .iter()
            .copied()
            .enumerate()
            .map(|(temp, backend::AssetRef { path, .. })| {
                let color = if *idx == temp {
                    Self::COLOR_FOCUS
                } else {
                    Self::COLOR_BASIC
                };

                (path, font_attrs.color(color))
            }).flat_map(|a| [a, ("\n", font_attrs.color(Self::COLOR_BASIC))]);

        buttons.set_rich_text(
            font_system, 
            spans, 
            glyphon::Shaping::Basic,
        );

        buttons.set_size(font_system, width as f32, height as f32);

        let region = glyphon::TextArea {
            buffer: buttons,
            left: 0.,
            top: 0.,
            scale: 1.,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            },
            default_color: glyphon::Color::rgb(255, 255, 255),
        };

        renderer.prepare(
            device,
            queue,
            font_system,
            atlas,
            glyphon::Resolution { width, height },
            Some(region),
            swash_cache,
        )
    }

    pub fn load(
        &mut self, 
        device: &wgpu::Device, 
        queue: &wgpu::Queue,
        bytes: &[u8],
        screen_resolution: backend::Size,
    ) -> Result<geom::Geometry<geom::FeatureVertex, geom::FeatureMetadata>, LoaderError> {
        let Self {
            slices,
            stacks,
            globe_radius, ..
        } = self;

        let features = str::from_utf8(bytes)
            .map_err(LoaderError::InvalidPath)?
            .parse::<geojson::GeoJson>()
            .map_err(LoaderError::InvalidGeoJson)?;

        let geojson::FeatureCollection { 
            features, .. 
        } = geojson::FeatureCollection::try_from(features)
            .map_err(LoaderError::InvalidGeoJson)?;

        let geometry = geom::Geometry::build_feature_geometry(
            device, 
            features.as_slice(),
            *slices, 
            *stacks,
            *globe_radius, 
        ).map_err(LoaderError::BrokenGeometry)?;

        self.prepare(device, queue, screen_resolution)
            .map_err(LoaderError::LabelFailure)?;

        Ok(geometry)
    }

    pub fn render<'p, 'a: 'p>(
        &'a self, 
        #[allow(unused_variables)]
        pass: &mut wgpu::RenderPass<'p>,
    ) -> Result<(), glyphon::RenderError> {
        let Self { 
            toggled,
            atlas, 
            renderer, .. 
        } = self;

        if *toggled { renderer.render(atlas, pass) } else { Ok(()) }
    }
}