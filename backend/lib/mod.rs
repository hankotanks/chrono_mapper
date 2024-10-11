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
pub enum Request<'a> {
    Fulfilled { path: &'a str, bytes: &'a [u8] },
    Failed(&'a str),
    Loading(&'a str),
}

#[derive(Debug)]
enum RequestInternal {
    Fulfilled { path: String, bytes: Vec<u8>, }, 
    Failed(String),
    #[cfg(target_arch = "wasm32")]
    Loading { path: String, root: String },
}

impl<'a> Into<Request<'a>> for &'a RequestInternal {
    fn into(self) -> Request<'a> {
        match self {
            RequestInternal::Fulfilled { path, bytes } => Request::Fulfilled { path, bytes },
            RequestInternal::Failed(path) => Request::Failed(path),
            #[cfg(target_arch = "wasm32")]
            RequestInternal::Loading { path, .. } => Request::Loading(path)
        }
    }
}

#[derive(Clone, Copy)]
pub enum AppEvent<'a> {
    Key {code: event::KeyCode, state: event::ElementState },
    Mouse { button: event::MouseButton, state: event::ElementState, cursor: Position },
    MouseScroll { delta: f32 },
    MouseScrollStopped,
    MouseMotion { x: f32, y: f32 },
    Resized(Size),
    Request(Request<'a>),
}

#[derive(Clone, Copy)]
pub struct AppData<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    event_proxy: &'a winit::event_loop::EventLoopProxy<RequestInternal>,
}

impl<'a> AppData<'a> {
    #[cfg(not(target_arch = "wasm32"))]
    const OUT_DIR: &'static str = env!("BACKEND_OUT_DIR");

    pub fn get_static_asset(&self, path: &str) -> std::io::Result<&[u8]> {
        use std::io::{Error, ErrorKind};
        use std::sync::OnceLock;
        use std::collections::HashMap;

        static STATIC: OnceLock<HashMap<&'static str, &'static [u8]>> = OnceLock::new();

        STATIC.get_or_init(|| HashMap::from_iter({
            generate().into_iter().map(|(tag, asset)| (tag, asset.data))
        })).get(path).copied().ok_or(Error::from(ErrorKind::NotFound))
    }

    pub fn get(&self, path: &str) -> std::io::Result<()> {
        let Self { event_proxy, .. } = self;

        #[cfg(target_arch = "wasm32")] {
            fn url() -> Result<String, wasm_bindgen::JsValue> {
                web_sys::window()
                    .ok_or(wasm_bindgen::JsValue::UNDEFINED)?
                    .location()
                    .href()
            }

            match url() {
                Ok(root) => {
                    let req = RequestInternal::Loading {
                        path: String::from(path),
                        root,
                    };

                    #[allow(unused_variables)]
                    let result = event_proxy.send_event(req);

                    #[cfg(feature = "logging")]
                    if let Err(e) = result { log::debug!("{e}"); }
                }, Err(_) => { /*  */ },
            }
        }

        #[cfg(not(target_arch = "wasm32"))] {
            use std::fs;
            use std::path::Path;

            #[allow(unused_variables)]
            let result = event_proxy.send_event({
                match fs::read(Path::new(Self::OUT_DIR).join(path)) {
                    Ok(bytes) => RequestInternal::Fulfilled { path: path.to_string(), bytes },
                    Err(_) => RequestInternal::Failed(String::from(path)),
                }
            });

            #[cfg(feature = "logging")]
            if let Err(e) = result { log::debug!("{e}"); }
        }

        Ok(())
    }
}

pub trait App {
    type Config: AppConfig;
    type RenderError: std::error::Error + Send + Sync + 'static;
    type UpdateError: std::error::Error + Send + Sync + 'static;

    fn new(
        config: Self::Config, 
        data: AppData<'_>,
    ) -> impl std::future::Future<Output = Result<Self, Self::UpdateError>>
        where Self: Sized;

    fn handle_event(
        &mut self, 
        data: AppData<'_>, 
        event: AppEvent<'_>,
    ) -> Result<bool, Self::UpdateError>;

    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> Result<(), Self::RenderError>;
}

pub trait AppConfig: Copy {
    fn surface_format(self) -> wgpu::TextureFormat;
}

struct Package<'a, C: AppConfig, A: App<Config = C>> {
    handler: UpdateHandler<'a, C, A>,
    event_loop: winit::event_loop::EventLoop<RequestInternal>,
}

impl<'a, C: AppConfig, A: App<Config = C>> Package<'a, C, A> {
    async fn new(config: C) -> anyhow::Result<Self> {
        let event_loop  = {
            winit::event_loop::EventLoopBuilder::with_user_event().build()?
        };

        let event_proxy = event_loop.create_proxy();

        let state = {
            state::State::new(&event_loop, config.surface_format()).await
        }?;

        let data = AppData {
            device: &state.device,
            queue: &state.queue,
            event_proxy: &event_proxy,
        };

        let app = (A::new(config, data).await)?;

        let handler = UpdateHandler { 
            app, 
            state, 
            event_proxy,
            #[cfg(target_arch = "wasm32")]
            asset_request_pending: None,
        };

        Ok(Self { handler, event_loop })
    }
}

struct UpdateHandler<'a, C: AppConfig, A: App<Config = C>> {
    app: A,
    state: state::State<'a>,
    event_proxy: winit::event_loop::EventLoopProxy<RequestInternal>,
    #[cfg(target_arch = "wasm32")]
    asset_request_pending: Option<String>,
}

impl<'a, C: AppConfig, A: App<Config = C>> UpdateHandler<'a, C, A> {
    fn process(
        &mut self, 
        event: winit::event::Event<RequestInternal>,
        event_target: &winit::event_loop::EventLoopWindowTarget<RequestInternal>,
    ) -> anyhow::Result<()> {
        let Self { 
            app, 
            state, 
            event_proxy, 
            #[cfg(target_arch = "wasm32")]
            asset_request_pending,
        } = self;

        #[cfg(target_arch = "wasm32")] {
            async fn req_bytes(url: &str) -> Result<Vec<u8>, wasm_bindgen::JsValue> {
                use wasm_bindgen::JsCast as _;

                let opts = web_sys::RequestInit::new();
                    opts.set_method("GET");
                    opts.set_mode(web_sys::RequestMode::Cors);

                let request = web_sys::Request::new_with_str_and_init(&url, &opts)?;

                let resp = web_sys::window()
                    .ok_or(wasm_bindgen::JsValue::UNDEFINED)?
                    .fetch_with_request(&request);

                let resp = (wasm_bindgen_futures::JsFuture::from(resp).await)?
                    .dyn_into::<web_sys::Response>()?
                    .text()?;
            
                let bytes = (wasm_bindgen_futures::JsFuture::from(resp).await)?
                    .as_string()
                    .ok_or(wasm_bindgen::JsValue::UNDEFINED)?
                    .into_bytes();

                Ok(bytes)
            }

            async fn req(
                proxy: winit::event_loop::EventLoopProxy<RequestInternal>,
                url: &str,
            ) -> Result<(), winit::event_loop::EventLoopClosed<RequestInternal>> {
                let path = String::from(url);

                proxy.send_event({
                    match req_bytes(url).await {
                        Ok(bytes) => RequestInternal::Fulfilled { path, bytes },
                        Err(_) => RequestInternal::Failed(path),
                    }
                })
            }

            if let Some(path) = asset_request_pending.take() {
                let event_proxy = event_proxy.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    // It's okay to discard this error
                    // Because it can only occur if the EventLoop has been closed
                    // Which causes the process to exit immediately
                    #[allow(unused_variables)]
                    let result = req(event_proxy, &path).await;
        
                    #[cfg(feature = "logging")]
                    if let Err(e) = result { log::debug!("{e}"); }
                });

                event_target.set_control_flow(winit::event_loop::ControlFlow::Wait);
            }
        }

        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::RedrawRequested,
                window_id,
            } if window_id == state.window.id() => {
                state.process_encoder::<A::RenderError, _>(|encoder, view| {
                    app.submit_passes(encoder, view)
                })?;
            },
            winit::event::Event::UserEvent(req) => {
                let event = AppEvent::Request(Into::<Request>::into(&req));

                let data = AppData {
                    device: &state.device,
                    queue: &state.queue,
                    event_proxy,
                };

                #[cfg(target_arch = "wasm32")]
                if let RequestInternal::Loading { path, root } = &req {
                    let mut root = root.clone(); root.push_str(&path);

                    let _ = asset_request_pending.insert(root);

                    event_target.set_control_flow(winit::event_loop::ControlFlow::Poll);
                }
                
                if app.handle_event(data, event)? {
                    state.window.request_redraw();
                }
            },
            event => {
                for event in state.run(event, event_target)? {
                    let data = AppData {
                        device: &state.device,
                        queue: &state.queue,
                        event_proxy,
                    };
                    
                    if app.handle_event(data, event)? {
                        state.window.request_redraw();
                    }
                }
            },
        }
    
        Ok(())
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
        
        #[cfg(not(target_arch = "wasm32"))] 
        simple_logger::SimpleLogger::new()
            .with_level(log::LevelFilter::Info)
            .init()
            .unwrap();
    }

    let Package {
        mut handler, event_loop,
    } = (Package::<'_, C, A>::new(config).await).map_err(|e| e.to_string())?;

    let err_outer = Rc::new(OnceCell::new());
    let err_inner = Rc::clone(&err_outer);
    event_loop.run(move |event, event_target| {
        if let Err(e) = handler.process(event, event_target) {
            event_target.exit();
            err_inner.get_or_init(|| e);
        }
    }).map_err(|e| e.to_string())?;

    if let Some(mut container) = Rc::into_inner(err_outer) {
        if let Some(e) = container.take() { Err(e.to_string())?; }
    }

    Ok(())
}