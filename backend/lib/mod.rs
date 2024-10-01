include!(concat!(env!("OUT_DIR"), "/generated.rs"));

mod state;

#[cfg(target_arch = "wasm32")]
mod web;

// contains wrappers over winit::event
// prevents `app` from requiring winit as a dependency
pub mod event {
    pub use winit::{
        keyboard::KeyCode, 
        event::MouseButton, 
        event::ElementState,
    };
}

use std::{cell, collections, future, io, rc};

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

#[derive(Debug)]
#[derive(Clone, Copy)]
pub enum AppEvent {
    Key {code: event::KeyCode, state: event::ElementState },
    Mouse { button: event::MouseButton, state: event::ElementState, cursor: Position },
    MouseScroll { delta: f32 },
    MouseScrollStopped,
    MouseMotion { x: f32, y: f32 },
    Resized(Size)
}

pub trait App {
    type Config: AppConfig;

    fn new(
        config: Self::Config,
        device: &wgpu::Device, 
        queue: &wgpu::Queue,
    ) -> impl future::Future<Output = anyhow::Result<Self>> where Self: Sized;

    fn update(
        &mut self, 
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> anyhow::Result<()>;

    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()>;

    fn handle_event(
        &mut self, 
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        assets: &Assets,
        event: AppEvent,
    ) -> bool;
}

pub trait AppConfig: Copy {
    fn surface_format(self) -> wgpu::TextureFormat;
}

struct Package<'a, C: AppConfig, A: App<Config = C>> {
    app: A,
    state: state::State<'a>,
    event_loop: winit::event_loop::EventLoop<Vec<u8>>,
    assets: Assets,
}

impl<'a, C: AppConfig, A: App<Config = C>> Package<'a, C, A> {
    async fn new(config: C) -> anyhow::Result<Self> {
        use winit::event_loop::{EventLoop, EventLoopBuilder};

        let event_loop: EventLoop<Vec<u8>> = EventLoopBuilder::with_user_event().build()?;

        let state = {
            state::State::new(&event_loop, config.surface_format()).await
        }?;

        let app = (A::new(config, &state.device, &state.queue).await)?;

        let assets = Assets(event_loop.create_proxy());

        Ok(Self { app, state, event_loop, assets })
    }
}

pub async fn start<C, A>(config: C) -> Result<(), String>
    where C: AppConfig, A: App<Config = C> {

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

    let Package { 
        mut app, 
        mut state, 
        event_loop,
        assets,
    } = (Package::<'_, C, A>::new(config).await)
        .map_err(|e| e.to_string())?;

    let err = rc::Rc::new(cell::OnceCell::new());
    let err_inner = rc::Rc::clone(&err);

    event_loop.run(move |event, event_target| {
        use winit::event::{Event, WindowEvent};

        if let Some(physical_size) = unsafe { VIEWPORT.take() } {
            state.resize(physical_size);

            let event = AppEvent::Resized(physical_size.into());

            if app.handle_event(&state.device, &state.queue, &assets, event) {
                state.window.request_redraw();
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } if window_id == state.window.id() => {
                if let Err(e) = state.process_encoder(|encoder, view| {
                    app.submit_passes(encoder, view)
                }) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }
            },
            Event::UserEvent(bytes) => {
                if let Err(e) = app.update(&state.device, &state.queue, &bytes) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }

                state.window.request_redraw();
            },
            event => match state.run(event, event_target) {
                Ok(Some(event)) => {
                    if app.handle_event(&state.device, &state.queue, &assets, event) {
                        state.window.request_redraw();
                    }
                },
                Ok(None) => { /*  */ },
                Err(e) => {
                    let _ = err_inner.get_or_init(|| e);
    
                    event_target.exit();
                },
            },
        }
    }).map_err(|e| e.to_string())?;

    if let Some(mut container) = rc::Rc::into_inner(err) {
        if let Some(e) = container.take() { Err(e.to_string())?; }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn update_canvas(
    w: wasm_bindgen::JsValue, h: wasm_bindgen::JsValue,
) -> Result<(), String> {
    unsafe fn update_canvas_inner(
        w: wasm_bindgen::JsValue, h: wasm_bindgen::JsValue,
    ) -> anyhow::Result<winit::dpi::PhysicalSize<u32>> {
        let width: u32 = w.as_string()
            .ok_or(web::WebError::new("parse canvas width"))?
            .parse()?;
    
        let height: u32 = h.as_string()
            .ok_or(web::WebError::new("parse canvas height"))?
            .parse()?;

        Ok(winit::dpi::PhysicalSize { width, height })
    }

    unsafe {
        let size = update_canvas_inner(w, h)
            .map_err(|e| e.to_string())?;

        let _ = VIEWPORT.insert(size);
    }

    Ok(())
}

static mut VIEWPORT: Option<winit::dpi::PhysicalSize<u32>> = None;

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

pub struct Assets(winit::event_loop::EventLoopProxy<Vec<u8>>); 

impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    const WORKSPACE_ROOT: &'static str = env!("WORKSPACE_ROOT");

    pub fn retrieve(path: &str) -> io::Result<&[u8]> {
        use once_cell::sync::Lazy;

        static STATIC: Lazy<collections::HashMap<&'static str, &'static [u8]>> = Lazy::new(|| {
            let mut assets = collections::HashMap::new();
            for (tag, asset) in generate().into_iter() {
                assets.insert(tag, asset.data);
            }
        
            assets
        });

        STATIC
            .get(path)
            .copied()
            .ok_or(io::Error::from(io::ErrorKind::NotFound))
    }

    pub fn request(&self, aref: AssetRef<'_>) {
        let Self(proxy) = self;

        let AssetRef { path, locator } = aref;

        match locator {
            AssetLocator::Local => {
                #[cfg(target_arch = "wasm32")] {
                    match web::url() {
                        Ok(mut url) => {
                            url.push_str(path);

                            let proxy = proxy.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                // TODO: Report failure to fetch
                                web::req(proxy, &url).await.unwrap()
                            });
                        },
                        Err(_) => { /*  */ },
                    }
                }

                #[cfg(not(target_arch = "wasm32"))] {
                    let path = std::path::Path::new(Self::WORKSPACE_ROOT)
                        .join(path);
                    
                    if let io::Result::Ok(bytes) = std::fs::read(path) {
                        let _ = proxy.send_event(bytes);
                    }
                }
            },
        }
    }
}