use crate::globe::util;

pub struct Basemap {
    pub buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    pub buffer_size: winit::dpi::PhysicalSize<u32>,
}

impl Basemap {
    pub fn from_bytes(
        bytes: &[u8],
        padding: winit::dpi::PhysicalSize<u32>,
    ) -> Result<Self, image::error::ImageError> {
        use image::GenericImageView as _;

        let buffer = image::load_from_memory(bytes)?
            .to_rgba8();

        let buffer_size = winit::dpi::PhysicalSize {
            width: buffer.width() - padding.width * 2,
            height: buffer.height() - padding.height * 2,
        };

        let buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = buffer.view(
            padding.width, padding.height, 
            buffer_size.width, buffer_size.height,
        ).to_image();

        Ok(Self { buffer, buffer_size })
    }

    pub fn with_features(
        mut self, 
        features: Vec<geojson::Feature>,
    ) -> Self {
        for (name, geometry) in features.iter().filter_map(util::validate_feature_properties) {
            let geojson::Geometry { value, .. } = geometry;

            if let geojson::Value::MultiPolygon(polygons) = value {
                for polygon in polygons {
                    if let Some(outer) = polygon.first() {
                        let mut points = outer
                            .iter()
                            .map(|vertex| imageproc::point::Point {
                                x: (((vertex[0]) / 180. + 1.) * 0.5 * self.buffer_size.width as f64).floor() as i32,
                                y: ((1. - (((vertex[1]) / 90. + 1.) * 0.5)) * self.buffer_size.height as f64).floor() as i32,
                            }).collect::<Vec<_>>();

                        points.dedup();

                        if points.len() > 2 {
                            imageproc::drawing::draw_antialiased_polygon_mut(
                                &mut self.buffer, 
                                &points[0..(points.len() - 1)], 
                                image::Rgba(util::hashable_to_rgba8(name)),
                                imageproc::pixelops::interpolate,
                            );
                        }
                    }
                }
            }
        }

        self
    }
}