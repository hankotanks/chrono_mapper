include!(concat!(env!("OUT_DIR"), "/generated.rs"));

mod state;

use std::{cell, collections, future, rc};

pub trait Harness {
    type Config: HarnessConfig;

    fn new(
        config: Self::Config,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        assets: collections::HashMap<&'static str, &'static [u8]>,
    ) -> impl future::Future<Output = anyhow::Result<Self>> where Self: Sized;

    fn update(
        &mut self, 
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<()>;

    fn submit_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
    ) -> anyhow::Result<()>;

    fn handle_event(
        &mut self, 
        event: winit::event::DeviceEvent,
    ) -> bool;

    fn handle_mouse_click(
        &mut self,
        button: winit::event::MouseButton,
        cursor: winit::dpi::PhysicalPosition<f32>,
    ) -> bool;

    fn handle_resize(
        &mut self, 
        size: winit::dpi::PhysicalSize<u32>,
        scale: f32,
    );
}

pub trait HarnessConfig: Copy {
    fn surface_format(self) -> wgpu::TextureFormat;
}

pub struct App<'a, Hc: HarnessConfig, H: Harness<Config = Hc>> {
    state: state::State<'a>,
    inner: H,
    event_loop: winit::event_loop::EventLoop<()>,
}

impl<'a, Hc: HarnessConfig, H: Harness<Config = Hc>> App<'a, Hc, H> {
    pub async fn new(
        config: Hc,
    ) -> Result<Self, String> where Self: Sized {
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

        async fn new_inner<'a, Hc: HarnessConfig, H: Harness<Config = Hc>>(
            config: Hc
        ) -> anyhow::Result<App<'a, Hc, H>> {
            let mut assets = collections::HashMap::new();
            for (tag, asset) in generate().into_iter() {
                assets.insert(tag, asset.data);
            }

            let surface_format = config.surface_format();

            let event_loop = winit::event_loop::EventLoop::new()?;

            let state = {
                state::State::new(&event_loop, surface_format).await
            }?;

            let inner = (H::new(config, &state.device, &state.queue, assets).await)?;
    
            Ok(App { state, inner, event_loop })
        }

        new_inner(config).await.map_err(|e| e.to_string())
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

    pub fn run(self) -> Result<(), String> {
        let Self { mut inner, mut state, event_loop } = self;

        let err = rc::Rc::new(cell::OnceCell::new());
        let err_inner = rc::Rc::clone(&err);

        event_loop.run(move |event, event_target| {
            use winit::event::{Event, WindowEvent};

            if let Some(physical_size) = unsafe { VIEWPORT.take() } {
                state.resize(physical_size);

                inner.handle_resize(
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
                    if inner.handle_event(event) {
                        state.window.request_redraw();
                    }
                },
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    window_id,
                } if window_id == state.window.id() => {
                    if let Err(e) = inner.update(&state.device, &state.queue) {
                        let _ = err_inner.get_or_init(|| e);

                        event_target.exit();
                    }

                    if let Err(e) = state.process_encoder(|encoder, view| {
                        inner.submit_passes(encoder, view)
                    }) {
                        let _ = err_inner.get_or_init(|| e);

                        event_target.exit();
                    }
                },
                event => {
                    match event {
                        Event::WindowEvent { 
                            window_id, 
                            event: WindowEvent::MouseInput {
                                button,
                                state: winit::event::ElementState::Pressed, ..
                            },
                        } if window_id == state.window.id() && state.cursor.is_some() => {
                            let winit::dpi::PhysicalPosition { x, y } = state.cursor.unwrap();

                            let winit::dpi::PhysicalSize { width, height } = state.window.inner_size();

                            let cursor = winit::dpi::PhysicalPosition {
                                x: (x / width as f32) * 2. - 1.,
                                y: (0.5 - (y / height as f32)) * 2.,
                            };

                            if inner.handle_mouse_click(button, cursor) {
                                state.window.request_redraw();
                            }
                        },
                        event => match state.run(event, event_target) {
                            Ok(Some(size)) => {
                                inner.handle_resize(size, state.window.scale_factor() as f32);
                            },
                            Ok(None) => { /*  */ },
                            Err(e) => {
                                let _ = err_inner.get_or_init(|| e);
                
                                event_target.exit();
                            },
                        }
                    }
                },
            }
        }).map_err(|e| e.to_string())?;

        if let Some(mut container) = rc::Rc::into_inner(err) {
            if let Some(e) = container.take() { Err(e.to_string())?; }
        }

        Ok(())
    }
}

pub static mut VIEWPORT: Option<winit::dpi::PhysicalSize<u32>> = None;