include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use std::{cell, collections, future, rc, sync};

use winit::dpi::PhysicalSize;

pub trait Harness {
    type Config: Copy;

    fn new<'a>(
        config: Self::Config, 
        assets: collections::HashMap<&'a str, &'a [u8]>,
        window: sync::Arc<winit::window::Window>,
    ) -> impl future::Future<Output = anyhow::Result<Self>> where Self: Sized;
    fn update(&mut self) -> anyhow::Result<()>;
    fn resize(&mut self, size: PhysicalSize<u32>);
    fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool;
}

pub struct App<T, H: Harness<Config = T>> {
    inner: H,
    event_loop: winit::event_loop::EventLoop<()>,
    window: sync::Arc<winit::window::Window>,
}

impl<T, H: Harness<Config = T>> App<T, H> {
    pub async fn new(
        config: T,
    ) -> anyhow::Result<Self> where Self: Sized {
        let mut assets = collections::HashMap::new();
        for (tag, asset) in generate().into_iter() {
            assets.insert(tag, asset.data);
        }

        let event_loop = winit::event_loop::EventLoop::new()?;
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        let window = sync::Arc::new({
            winit::window::WindowBuilder::new().build(&event_loop)?
        });

        let app = Self {
            inner: H::new(config, assets, window.clone())
                .await
                .map_err(anyhow::Error::msg)?,
            event_loop,
            window,
        };

        Ok(app)
    }

    pub fn run(self) -> anyhow::Result<()> {
        let Self { mut inner, event_loop, window, .. } = self;

        fn run_internal<T, H: Harness<Config = T>>(
            inner: &mut H,
            window: &winit::window::Window,
            event: winit::event::Event<()>,
            event_target: &winit::event_loop::EventLoopWindowTarget<()>,
        ) -> anyhow::Result<()> {
            use winit::event::{Event, WindowEvent, KeyEvent, ElementState};

            use winit::keyboard::{Key, NamedKey};

            match event {
                Event::WindowEvent { 
                    event, 
                    window_id, .. 
                } if window_id == window.id() => {
                    match event {
                        WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                            event: KeyEvent {
                                state: ElementState::Pressed,
                                logical_key: Key::Named(NamedKey::Escape), ..
                            }, ..
                        } => event_target.exit(),
                        WindowEvent::Resized(physical_size) => {
                            inner.resize(physical_size);

                            window.request_redraw();
                        },
                        WindowEvent::RedrawRequested => {
                            inner.update()?;
                        },
                        _ => { /*  */ },
                    }
                },
                Event::DeviceEvent { event, .. } => {
                    if inner.handle_event(event) { window.request_redraw(); }
                },
                _ => { /*  */ },
            }

            Ok(())
        }

        let err = rc::Rc::new(cell::OnceCell::new());
        let err_inner = rc::Rc::clone(&err);

        event_loop.run(move |event, event_target| {
            if let Some(physical_size) = unsafe { VIEWPORT.take() } {
                inner.resize(physical_size);
            }
            
            if let Err(e) = run_internal(&mut inner, &window, event, event_target) {
                let _ = err_inner.get_or_init(|| e);

                event_target.exit();
            }
        })?;

        if let Some(mut container) = rc::Rc::into_inner(err) {
            if let Some(e) = container.take() {
                Err(e)?;
            }
        }

        Ok(())
    }
}

pub static mut VIEWPORT: Option<PhysicalSize<u32>> = None;