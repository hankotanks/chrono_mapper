include!(concat!(env!("OUT_DIR"), "/generated.rs"));

// re-export macros from backend
pub use backend_macros::*;

// re-export log when the implemented asks for it
#[cfg(feature = "logging")]
pub mod log {
    pub use log::*;
}


// contains wrappers over winit::event
// prevents `app` from requiring winit as a dependency
pub mod event {
    pub use winit::{
        keyboard::KeyCode, 
        event::MouseButton, 
        event::ElementState,
    };
}

// same situation as event
pub mod wgpu {
    pub use wgpu::*;
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    pub mod wasm_bindgen {
        pub use wasm_bindgen::*;
    }
    
    pub mod wasm_bindgen_futures {
        pub use wasm_bindgen_futures::*;
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    pub mod pollster { 
        pub use pollster::block_on; 
    }
}

mod state;

use std::error;

#[derive(Clone, Copy)]
#[derive(Debug)]
pub struct Position { pub x: f32, pub y: f32 }

impl From<winit::dpi::PhysicalPosition<f32>> for Position {
    fn from(value: winit::dpi::PhysicalPosition<f32>) -> Self {
        let winit::dpi::PhysicalPosition { x, y } = value;

        Self { x, y } 
    }
}

#[derive(Default)]
#[derive(Clone, Copy)]
#[derive(Debug)]
pub struct Size { pub width: u32, pub height: u32 }

impl From<winit::dpi::PhysicalSize<u32>> for Size {
    fn from(value: winit::dpi::PhysicalSize<u32>) -> Self {
        let winit::dpi::PhysicalSize { width, height } = value;

        Self { width, height } 
    }
}

#[derive(Clone, Copy)]
#[derive(Debug)]
pub enum AppEvent {
    Key {code: event::KeyCode, state: event::ElementState },
    Mouse { button: event::MouseButton, state: event::ElementState, cursor: Position },
    MouseScroll { delta: f32, cursor: Position },
    MouseScrollStopped,
    MouseMotion { x: f32, y: f32 },
    Resized(Size)
}

pub trait App {
    type Config: AppConfig;
    type SubmissionError: error::Error + Send + Sync + 'static;
    type UpdateError: error::Error + Send + Sync + 'static;

    fn new(
        config: Self::Config,
        device: &wgpu::Device, queue: &wgpu::Queue,
        assets: Assets,
    ) -> impl std::future::Future<Output = anyhow::Result<Self>> 
        where Self: Sized;

    fn update(
        &mut self, 
        device: &wgpu::Device, queue: &wgpu::Queue,
        bytes: &[u8],
        asset_path: &str,
    ) -> Result<(), Self::UpdateError>;

    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> Result<(), Self::SubmissionError>;

    fn handle_event(
        &mut self, 
        device: &wgpu::Device, queue: &wgpu::Queue,
        assets: Assets, 
        event: AppEvent,
    ) -> bool;
}

pub trait AppConfig: Copy {
    fn surface_format(self) -> wgpu::TextureFormat;
}

struct Package<'a, C: AppConfig, A: App<Config = C>> {
    app: A,
    state: state::State<'a>,
    event_loop: winit::event_loop::EventLoop<Request>,
}

impl<'a, C: AppConfig, A: App<Config = C>> Package<'a, C, A> {
    async fn new(config: C) -> anyhow::Result<Self> {
        let event_loop  = {
            use winit::event_loop::EventLoopBuilder;

            EventLoopBuilder::with_user_event().build()?
        };

        let state = {
            state::State::new(&event_loop, config.surface_format()).await
        }?;

        let assets = Assets {
            proxy: event_loop.create_proxy(),
            loading: false,
        };

        let app = {
            A::new(config, &state.device, &state.queue, assets).await
        }?;

        Ok(Self { app, state, event_loop })
    }
}

pub async fn start<C, A>(config: C) -> Result<(), String>
    where C: AppConfig, A: App<Config = C> {

    use std::rc::Rc;
    use std::cell::OnceCell;

    #[cfg(feature = "logging")] {
        #[cfg(target_arch = "wasm32")] {
            console_error_panic_hook::set_once();
            wasm_logger::init(wasm_logger::Config::default());
        }
        
        #[cfg(not(target_arch = "wasm32"))] {
            simple_logger::SimpleLogger::new()
                .with_level(log::LevelFilter::Info)
                .init()
                .unwrap();
        }
    }

    let Package { 
        mut app, 
        mut state, 
        event_loop,
    } = (Package::<'_, C, A>::new(config).await)
        .map_err(|e| e.to_string())?;

    let proxy = event_loop.create_proxy();

    let mut loading = false;

    let err = Rc::new(OnceCell::new());
    let err_inner = Rc::clone(&err);

    event_loop.run(move |event, event_target| {
        use winit::event::{Event, WindowEvent};

        match event {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } if window_id == state.window.id() => {
                if let Err(e) = state.process_encoder::<A::SubmissionError, _>(|encoder, view| {
                    app.submit_passes(encoder, view)
                }) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }
            },
            Event::UserEvent(req) => {
                match req {
                    Request::Loading => loading = true,
                    Request::Fulfilled { bytes, path } => {
                        loading = false;

                        #[cfg(feature = "logging")]
                        log::debug!("finished loading asset [{}]", &path);

                        if let Err(e) = app.update(&state.device, &state.queue, &bytes, &path) {
                            let _ = err_inner.get_or_init(|| Into::<anyhow::Error>::into(e));
        
                            event_target.exit();
                        }
                    },
                    Request::Failed => loading = false,
                }

                state.window.request_redraw();
            },
            event => match state.run(event, event_target) {
                Ok(events) => {
                    for event in events {
                        let assets = Assets { proxy: proxy.clone(), loading };
                        if app.handle_event(&state.device, &state.queue, assets, event) {
                            state.window.request_redraw();
                        }
                    }
                },
                Err(e) => {
                    let _ = err_inner.get_or_init(|| e);
    
                    event_target.exit();
                },
            },
        }
    }).map_err(|e| e.to_string())?;

    if let Some(mut container) = Rc::into_inner(err) {
        if let Some(e) = container.take() { Err(e.to_string())?; }
    }

    Ok(())
}

#[derive(Clone, Copy)]
pub enum AssetLocator { 
    // relative to the project root
    // on web, this is base URL
    Local,
}

#[derive(Clone, Copy)]
pub struct AssetRef<'a> {
    pub path: &'a str,
    pub locator: AssetLocator,
}

pub enum Request {
    Loading,
    Fulfilled { path: String, bytes: Vec<u8> }, 
    Failed,
}

pub struct Assets {
    proxy: winit::event_loop::EventLoopProxy<Request>,
    loading: bool,
}

impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    const WORKSPACE_ROOT: &'static str = env!("WORKSPACE_ROOT");

    pub fn retrieve(path: &str) -> std::io::Result<&[u8]> {
        use std::io::{Error, ErrorKind};
        use std::sync::OnceLock;
        use std::collections::HashMap;

        static STATIC: OnceLock<HashMap<&'static str, &'static [u8]>> = OnceLock::new();

        STATIC.get_or_init(|| {
            let mut assets = HashMap::new();
            for (tag, asset) in generate().into_iter() {
                assets.insert(tag, asset.data);
            }; assets
        }).get(path).copied().ok_or(Error::from(ErrorKind::NotFound))
    }

    pub fn request(&self, aref: AssetRef<'_>) -> std::io::Result<()> {
        let Self { proxy, loading } = self;

        if *loading {
            use std::io::{Error, ErrorKind};
            
            return Err(Error::from(ErrorKind::Interrupted));
        }

        let AssetRef { path, locator } = aref;

        match locator {
            AssetLocator::Local => {
                #[cfg(target_arch = "wasm32")] {
                    fn url() -> anyhow::Result<String> {
                        web_sys::window()
                            .ok_or(state::WebError::new("obtain window"))?
                            .location()
                            .href()
                            .map_err(|_| state::WebError::new("query website's base url"))
                    }

                    async fn req_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
                        use wasm_bindgen::JsCast as _;
                    
                        let opts = web_sys::RequestInit::new();
                            opts.set_method("GET");
                            opts.set_mode(web_sys::RequestMode::Cors);
                    
                        let request = web_sys::Request::new_with_str_and_init(&url, &opts)
                            .map_err(|_| state::WebError::new("initialize request"))?;
                    
                        let window = web_sys::window()
                            .ok_or(state::WebError::new("obtain window"))?;
                    
                        let resp = window.fetch_with_request(&request);
                        let resp = wasm_bindgen_futures::JsFuture::from(resp)
                            .await
                            .map_err(|_| state::WebError::new("fetch data"))?
                            .dyn_into::<web_sys::Response>()
                            .map_err(|_| state::WebError::new("cast response"))?
                            .text()
                            .map_err(|_| state::WebError::new("get response body"))?;
                    
                        let bytes = wasm_bindgen_futures::JsFuture::from(resp)
                            .await
                            .map_err(|_| state::WebError::new("get response body"))?
                            .as_string()
                            .unwrap()
                            .into_bytes();

                        Ok(bytes)
                    }

                    async fn req(
                        proxy: winit::event_loop::EventLoopProxy<Request>,
                        url: &str,
                    ) -> anyhow::Result<()> {
                        let path = String::from(url);

                        let retr = match req_bytes(url).await {
                            Ok(bytes) => Request::Fulfilled { path, bytes },
                            Err(_) => Request::Failed,
                        };

                        proxy.send_event(retr)
                            .map_err(|_| state::WebError::new("serve data to event loop"))
                    }

                    match url() {
                        Ok(mut url) => {
                            url.push_str(path);

                            #[allow(unused_variables)]
                            let result = proxy.send_event(Request::Loading);

                            #[cfg(feature = "logging")]
                            if let Err(e) = result { log::debug!("{e}"); }

                            let proxy = proxy.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                // It's okay to discard this error
                                // Because it can only occur if the EventLoop has been closed
                                // Which causes the process to exit immediately
                                #[allow(unused_variables)]
                                let result = req(proxy, &url).await;

                                #[cfg(feature = "logging")]
                                if let Err(e) = result { log::debug!("{e}"); }
                            });
                        },
                        Err(_) => { /*  */ },
                    }
                }

                #[cfg(not(target_arch = "wasm32"))] {
                    use std::{path, fs};

                    let path_full = path::Path::new(Self::WORKSPACE_ROOT)
                        .join(path);

                    let retr = match fs::read(path_full) {
                        Ok(bytes) => Request::Fulfilled { path: path.to_string(), bytes },
                        Err(_) => Request::Failed,
                    };

                    #[allow(unused_variables)]
                    let result = proxy.send_event(retr);

                    #[cfg(feature = "logging")]
                    if let Err(e) = result { log::debug!("{e}"); }
                }
            },
        }

        Ok(())
    }
}