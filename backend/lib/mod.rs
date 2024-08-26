include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use std::{cell, collections, future, io, rc, sync};

use winit::dpi::PhysicalSize;

pub trait Harness {
    type Config: Copy;

    fn new<'a>(
        config: Self::Config, 
        assets: collections::HashMap<&'a str, &'a [u8]>,
        window: &winit::window::Window,
    ) -> impl future::Future<Output = anyhow::Result<Self>> where Self: Sized;
    fn update(&mut self) -> anyhow::Result<()>;
    fn resize(&mut self, size: PhysicalSize<u32>);
    fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool;
}

pub struct App<T, H: Harness<Config = T>> {
    inner: sync::Arc<sync::Mutex<H>>,
    event_loop: winit::event_loop::EventLoop<()>,
    window: winit::window::Window,
    receiver: sync::mpsc::Receiver<PhysicalSize<u32>>,
}

impl<T, H: Harness<Config = T>> App<T, H> {
    pub async fn new(
        config: T,
    ) -> anyhow::Result<(Self, sync::mpsc::Sender<PhysicalSize<u32>>)> 
        where Self: Sized {

        let mut assets = collections::HashMap::new();
        for (tag, asset) in generate().into_iter() {
            assets.insert(tag, asset.data);
        }

        let event_loop = winit::event_loop::EventLoop::new()?;
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        let window = winit::window::WindowBuilder::new()
            .build(&event_loop)?;

        let (sender, receiver) = sync::mpsc::channel();

        let app = Self {
            inner: sync::Arc::new(sync::Mutex::new({
                H::new(config, assets, &window)
                    .await
                    .map_err(anyhow::Error::msg)?
            })),
            event_loop,
            window,
            receiver,
        };

        Ok((app, sender))
    }

    pub fn run(self) -> anyhow::Result<()> {
        let Self {
            inner,
            event_loop,
            window,
            receiver, ..
        } = self;

        fn run_internal<T, H: Harness<Config = T>>(
            inner: sync::Arc<sync::Mutex<H>>,
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
                            inner
                                .lock()
                                .map_err(|_| io::Error::from(io::ErrorKind::Other))?
                                .resize(physical_size);

                            window.request_redraw();
                        },
                        WindowEvent::RedrawRequested => {
                            inner
                                .lock()
                                .map_err(|_| io::Error::from(io::ErrorKind::Other))?
                                .update()?;
                        },
                        _ => { /*  */ },
                    }
                },
                Event::DeviceEvent { event, .. } => {
                    log::warn!("testing");
                    let should_update = inner
                        .lock()
                        .map_err(|_| io::Error::from(io::ErrorKind::Other))?
                        .handle_event(event);

                    if should_update { window.request_redraw(); }
                },
                _ => { /*  */ },
            }

            Ok(())
        }

        let err = rc::Rc::new(cell::OnceCell::new());
        
        let err_inner = rc::Rc::clone(&err);
        event_loop.run(move |event, event_target| {
            match receiver.try_recv() {
                Ok(physical_size) => match inner.lock() {
                    Ok(mut inner) => inner.resize(physical_size),
                    Err(..) => {
                        let _ = err_inner.get_or_init(|| anyhow::Error::from({
                            io::Error::from(io::ErrorKind::Other)
                        }));

                        event_target.exit();
                    },
                },
                Err(sync::mpsc::TryRecvError::Disconnected) => {
                    let _ = err_inner.get_or_init(|| anyhow::Error::from({
                        sync::mpsc::TryRecvError::Disconnected
                    }));

                    event_target.exit();

                },
                _ => { /*  */ },
            }
            if let Err(e) = run_internal(inner.clone(), &window, event, event_target) {
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