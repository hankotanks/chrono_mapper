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
}