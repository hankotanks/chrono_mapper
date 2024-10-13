backend::init!(App, Config => Config);

#[derive(Clone, Copy)]
pub struct Config;

impl backend::AppConfig for Config {
    fn surface_format(self) -> backend::wgpu::TextureFormat {
        backend::wgpu::TextureFormat::Rgba8Unorm
    }
    
    fn window_title<'a>(self) -> &'a str {
        "ChronoMapper"
    }
}

pub struct App(bool);

impl backend::App for App {
    type Config = Config;
    type RenderError = std::io::Error;
    type UpdateError = std::io::Error;

    async fn new(
        _config: Self::Config, 
        _data: backend::AppData<'_>,
    ) -> Result<Self, Self::RenderError> where Self: Sized {
        println!("{:?}", std::env::var("BACKEND_OUT_DIR"));
        Ok(Self(false))
    }

    fn submit_passes(
        &mut self,
        _encoder: &mut backend::wgpu::CommandEncoder,
        _surface: &backend::wgpu::TextureView,
    ) -> Result<(), Self::RenderError> {
        Ok(())
    }

    fn handle_event(
        &mut self, 
        data: backend::AppData<'_>,
        event: backend::AppEvent,
    ) -> Result<bool, Self::UpdateError> {
        let Self(requested) = self;
        if !(*requested) {
            *requested = true;
            data.get("features/world_100.geojson")?;
        }

        #[cfg(feature = "logging")]
        if let backend::AppEvent::Request(backend::Request { path, state }) = event {
            match state {
                backend::RequestState::Fulfilled(bytes) => //
                    backend::log::warn!("fulfilled asset request {}:\n{:?}", path, bytes),
                backend::RequestState::Failed => //
                    backend::log::warn!("failed to fulfill asset request {path}"),
                backend::RequestState::Loading => //
                    backend::log::warn!("started loading asset {path}"),
            }
        }
        
        if let backend::AppEvent::Key { 
            state: backend::event::ElementState::Released, ..  
        } = event {
            #[cfg(feature = "logging")]
            backend::log::warn!("{:?}", data.get_static_asset("test")); 
        }

        Ok(false)
    }
}