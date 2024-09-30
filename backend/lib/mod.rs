include!(concat!(env!("OUT_DIR"), "/generated.rs"));

mod state;

use std::{cell, collections, fs, future, path, rc};

pub trait App {
    type Config: AppConfig;

    fn new(
        config: Self::Config,
        device: &wgpu::Device, queue: &wgpu::Queue,
    ) -> impl future::Future<Output = anyhow::Result<Self>> where Self: Sized;

    fn update(
        &mut self, 
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> impl future::Future<Output = anyhow::Result<()>>;

    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()>;

    fn handle_event(
        &mut self, 
        event: winit::event::DeviceEvent,
    ) -> bool;

    fn handle_resize(
        &mut self, 
        size: winit::dpi::PhysicalSize<u32>,
        scale: f32,
    );
}

pub trait AppConfig: Copy {
    fn surface_format(self) -> wgpu::TextureFormat;
}

struct Package<'a, C: AppConfig, A: App<Config = C>> {
    app: A,
    state: state::State<'a>,
    event_loop: winit::event_loop::EventLoop<()>,
}

impl<'a, C: AppConfig, A: App<Config = C>> Package<'a, C, A> {
    async fn new(config: C) -> anyhow::Result<Self> {
        let event_loop = winit::event_loop::EventLoop::new()?;

        let state = {
            state::State::new(&event_loop, config.surface_format()).await
        }?;

        let app = (A::new(config, &state.device, &state.queue).await)?;

        Ok(Self { app, state, event_loop })
    }
}

pub async fn start<C, A>(config: C) -> Result<(), String>
    where C: AppConfig, A: App<Config = C> + 'static {

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
    } = (Package::<'_, C, A>::new(config).await)
        .map_err(|e| e.to_string())?;

    let err = rc::Rc::new(cell::OnceCell::new());
    let err_inner = rc::Rc::clone(&err);


    use winit::platform::web::EventLoopExtWebSys as _;

    event_loop.spawn(move |event, event_target| {
        use winit::event::{Event, WindowEvent};

        if let Some(physical_size) = unsafe { VIEWPORT.take() } {
            state.resize(physical_size);

            app.handle_resize(
                winit::dpi::PhysicalSize {
                    width: state.surface_config.width,
                    height: state.surface_config.height,
                },
                state.window.scale_factor() as f32,
            );
        }

        match event {
            Event::DeviceEvent { 
                event, .. 
            } if state.window.has_focus() => {
                if app.handle_event(event) {
                    state.window.request_redraw();
                }
            },
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } if window_id == state.window.id() => {
                if let Err(e) = futures::executor::block_on(app.update(&state.device, &state.queue)) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }

                if let Err(e) = state.process_encoder(|encoder, view| {
                    app.submit_passes(encoder, view)
                }) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }
            },
            event => match state.run(event, event_target) {
                Ok(Some(size)) => {
                    app.handle_resize(size, state.window.scale_factor() as f32);
                },
                Ok(None) => { /*  */ },
                Err(e) => {
                    let _ = err_inner.get_or_init(|| e);
    
                    event_target.exit();
                },
            },
        }
    });

    /*
    event_loop.run(move |event, event_target| {
        use winit::event::{Event, WindowEvent};

        if let Some(physical_size) = unsafe { VIEWPORT.take() } {
            state.resize(physical_size);

            app.handle_resize(
                winit::dpi::PhysicalSize {
                    width: state.surface_config.width,
                    height: state.surface_config.height,
                },
                state.window.scale_factor() as f32,
            );
        }

        match event {
            Event::DeviceEvent { 
                event, .. 
            } if state.window.has_focus() => {
                if app.handle_event(event) {
                    state.window.request_redraw();
                }
            },
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } if window_id == state.window.id() => {
                if let Err(e) = futures::executor::block_on(app.update(&state.device, &state.queue)) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }

                if let Err(e) = state.process_encoder(|encoder, view| {
                    app.submit_passes(encoder, view)
                }) {
                    let _ = err_inner.get_or_init(|| e);

                    event_target.exit();
                }
            },
            event => match state.run(event, event_target) {
                Ok(Some(size)) => {
                    app.handle_resize(size, state.window.scale_factor() as f32);
                },
                Ok(None) => { /*  */ },
                Err(e) => {
                    let _ = err_inner.get_or_init(|| e);
    
                    event_target.exit();
                },
            },
        }
    }).map_err(|e| e.to_string())?;*/

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
            .ok_or(state::err::WebError::new("parse canvas width"))?
            .parse()?;
    
        let height: u32 = h.as_string()
            .ok_or(state::err::WebError::new("parse canvas height"))?
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
    Static, 
    // relative to the project root
    // on web, this is base URL
    Local, 
    // performs a GET for the resource
    External,
 }

#[derive(Clone, Copy)]
pub struct AssetRef<'a> {
    pub path: &'a str,
    pub locator: AssetLocator,
}

pub struct Assets; impl Assets {
    #[cfg(not(target_arch = "wasm32"))]
    const WORKSPACE_ROOT: &'static str = env!("WORKSPACE_ROOT");

    pub async fn retrieve<'a>(aref: AssetRef<'a>) -> Option<&'static [u8]> {
        use once_cell::sync::Lazy;

        static STATIC: Lazy<collections::HashMap<&'static str, &'static [u8]>> = Lazy::new(|| {
            let mut assets = collections::HashMap::new();
            for (tag, asset) in generate().into_iter() {
                assets.insert(tag, asset.data);
            }
        
            assets
        });

        static mut DYNAMIC: Lazy<collections::HashMap<String, Vec<u8>>> = Lazy::new(|| {
            collections::HashMap::new()
        });

        let AssetRef { path, locator } = aref;

        match locator {
            AssetLocator::Static if STATIC.contains_key(path) => Some(STATIC[path]),
            AssetLocator::Local => {
                #[cfg(target_arch = "wasm32")] {
                    fn get_base_url() -> Result<String, state::err::WebError> {
                        web_sys::window()
                            .ok_or(state::err::WebError::new("obtain window"))?
                            .location()
                            .href()
                            .map_err(|_| state::err::WebError::new("query website's base url"))
                    }

                    match get_base_url() {
                        Ok(mut url) => {
                            url.push_str(path);

                            log::info!("{:?}", url);

                            let _ = req(&url).await;

                            None
                        },
                        Err(_) => None
                    }
                }

                #[cfg(not(target_arch = "wasm32"))] unsafe {
                    log::warn!("DEPLOYED: native");

                    match DYNAMIC.get(path) {
                        Some(bytes) => Some(bytes.as_slice()),
                        None => {
                            let loc = path::Path::new(Self::WORKSPACE_ROOT).join(path);

                            match fs::read(loc) {
                                Ok(bytes) => {
                                    DYNAMIC.insert(path.to_string(), bytes);
                                    DYNAMIC.get(path).map(|b| b.as_slice())
                                },
                                Err(_) => None,
                            }
                        },
                    }
                }
            },
            _ => None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn req(url: &str) -> Result<&[u8], wasm_bindgen::prelude::JsValue> {
    use wasm_bindgen::JsCast as _;

    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(&url, &opts)?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    assert!(resp_value.is_instance_of::<Response>());
    let resp: Response = resp_value.dyn_into().unwrap();

    let json = JsFuture::from(resp.json()?).await?;

    println!("{:?}", json);

    Ok(&[])
}