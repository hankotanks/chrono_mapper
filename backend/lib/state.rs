use std::sync;

#[cfg(target_arch = "wasm32")]
use super::web;

#[cfg(target_arch = "wasm32")]
fn surface_config_update(
    config: &mut wgpu::SurfaceConfiguration, 
    limits: wgpu::Limits,
    size: winit::dpi::PhysicalSize<u32>,
) {
    fn nearest_power_of_two(mut n: u32) -> u32 {
        if n == 0 {
            1
        } else if n & (n - 1) == 0 {
            n
        } else {
            n -= 1;
            n |= n >> 1;
            n |= n >> 2;
            n |= n >> 4;
            n |= n >> 8;
            n |= n >> 16;
            n += 1;
            n >> 1
        }
    }

    let wgpu::SurfaceConfiguration {
        width,
        height, ..
    } = config;

    *width = nearest_power_of_two(size.width)
        .clamp(1, limits.max_texture_dimension_2d);

    *height = nearest_power_of_two(size.height)
        .clamp(1, limits.max_texture_dimension_2d);
}

#[cfg(not(target_arch = "wasm32"))]
fn surface_config_update(
    config: &mut wgpu::SurfaceConfiguration, 
    limits: wgpu::Limits,
    size: winit::dpi::PhysicalSize<u32>,
) {
    let wgpu::SurfaceConfiguration {
        width,
        height, ..
    } = config;

    // 0-sized textures are not allowed
    *width = size.width.clamp(1, limits.max_texture_dimension_2d);

    // wgpu::Limits::max_texture_dimension_2d applies to both dimensions
    *height = size.height.clamp(1, limits.max_texture_dimension_2d);
}

pub struct State<'a> {
    pub window: sync::Arc<winit::window::Window>,
    pub required_limits: wgpu::Limits,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub surface: wgpu::Surface<'a>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub cursor: Option<winit::dpi::PhysicalPosition<f32>>,
}

impl<'a> State<'a> {
    pub async fn new(
        event_loop: &winit::event_loop::EventLoop<Vec<u8>>,
        surface_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        #[allow(non_snake_case)]
        let LIMITS = wgpu::Limits::downlevel_webgl2_defaults();

        let window = sync::Arc::new({
            winit::window::WindowBuilder::new().build(event_loop)?
        });

        fn create_surface_target<'a>(
            #[allow(unused_variables)] window: sync::Arc<winit::window::Window>,
        ) -> anyhow::Result<wgpu::SurfaceTarget<'a>> {
            #[cfg(target_arch="wasm32")] {
                use wasm_bindgen::JsCast as _;

                use winit::platform::web::WindowExtWebSys as _;

                let document = web_sys::window()
                    .ok_or(web::WebError::new("obtain window"))?
                    .document()
                    .ok_or(web::WebError::new("obtain document"))?;

                let elem: web_sys::Element = window
                    .as_ref()
                    .canvas()
                    .ok_or(web::WebError::new("create canvas"))?
                    .into();

                // Insert the canvas into the body
                document.body()
                    .ok_or(web::WebError::new("obtain body"))?
                    .append_child(&elem.clone().into())
                    .map_err(|_| web::WebError::new("append canvas to body"))?;

                let handle = elem.dyn_into::<web_sys::HtmlCanvasElement>()
                    .map_err(|_| web::WebError::new("reference render canvas"))?;

                Ok(wgpu::SurfaceTarget::Canvas(handle))
            }
            
            #[cfg(not(target_arch = "wasm32"))] {
                Ok(wgpu::SurfaceTarget::Window(Box::new(window)))
            }
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::default()
        });

        let surface = instance.create_surface({
            create_surface_target(sync::Arc::clone(&window))?
        })?;

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let device_desc = wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: LIMITS.clone(),
        };

        let (device, queue) = adapter
            .request_device(&device_desc, None)
            .await
            .unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);

        let wgpu::SurfaceCapabilities {
            present_modes,
            alpha_modes, ..
        } = surface_capabilities;

        // Construct the surface configuration
        let mut surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: 0,
            height: 0,
            present_mode: present_modes[0],
            alpha_mode: alpha_modes[0],
            view_formats: vec![surface_format],
            desired_maximum_frame_latency: 1,
        };

        surface_config_update(
            &mut surface_config,
            LIMITS.clone(),
            window.inner_size(),
        );

        // Configure the surface (no longer platform-specific)
        surface.configure(&device, &surface_config);

        Ok(Self {
            window,
            required_limits: LIMITS.clone(),
            queue,
            device,
            surface,
            surface_config,
            cursor: None,
        })
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let Self {
            required_limits,
            device,
            surface,
            surface_config, ..
        } = self;

        surface_config_update(
            surface_config, 
            required_limits.clone(), 
            size
        );

        surface.configure(device, surface_config);
    }

    pub fn run(
        &mut self, 
        event: winit::event::Event<Vec<u8>>,
        event_target: &winit::event_loop::EventLoopWindowTarget<Vec<u8>>,
    ) -> anyhow::Result<Option<crate::AppEvent>> {
        use winit::event::{Event, WindowEvent, KeyEvent, ElementState};

        use winit::keyboard::{Key, NamedKey};

        match event {
            Event::WindowEvent { 
                window_id, 
                event: WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                    event: KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Escape), ..
                    }, ..
                }
            } if window_id == self.window.id() => {
                event_target.exit();

                Ok(None)
            },
            Event::WindowEvent { 
                event: WindowEvent::Resized(physical_size), 
                window_id, .. 
            } if window_id == self.window.id() => {
                let Self {
                    window,
                    required_limits,
                    device,
                    surface_config, 
                    surface, ..
                } = self;

                surface_config_update(
                    surface_config, 
                    required_limits.clone(), 
                    physical_size
                );

                surface.configure(device, surface_config);

                window.request_redraw();

                let size = crate::Size {
                    width: surface_config.width,
                    height: surface_config.height,
                };

                Ok(Some(crate::AppEvent::Resized(size)))
            },
            Event::WindowEvent { 
                event: WindowEvent::CursorMoved { position, .. }, 
                window_id, .. 
            } if window_id == self.window.id() => {
                let _ = self.cursor.insert(position.cast());

                Ok(None)
            },
            Event::WindowEvent { 
                event: WindowEvent::CursorLeft { .. }, 
                window_id, .. 
            } if window_id == self.window.id() => {
                let _ = self.cursor.take();

                Ok(None)
            },
            Event::WindowEvent { 
                event: winit::event::WindowEvent::KeyboardInput { 
                    event: winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(code),
                        state, ..
                    }, .. 
                }, window_id, .. 
            } if window_id == self.window.id() => {
                Ok(Some(crate::AppEvent::Key { code, state }))
            },
            Event::WindowEvent { 
                event: winit::event::WindowEvent::MouseInput { 
                    button, 
                    state, .. 
                }, window_id, .. 
            } if window_id == self.window.id() => match self.cursor {
                Some(cursor) => {
                    let cursor = crate::Position::from(cursor);

                    Ok(Some(crate::AppEvent::Mouse { button, state, cursor }))
                },
                None => Ok(None),
            },
            Event::DeviceEvent {
                event: winit::event::DeviceEvent::MouseMotion { 
                    delta: (x, y),
                }, ..
            } => {
                Ok(Some(crate::AppEvent::MouseMotion { x: x as f32, y: y as f32 }))
            },
            Event::DeviceEvent {
                event: winit::event::DeviceEvent::MouseWheel { 
                    delta,
                }, ..
            } => {
                let delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(
                        winit::dpi::PhysicalPosition { y: scroll, .. }
                    ) => (self.window.scale_factor() * scroll) as f32 / 270.,
                } * -1.;

                Ok(Some(crate::AppEvent::MouseScroll { delta }))
            },
            _ => Ok(None),
        }
    }

    pub fn process_encoder<F>(&self, mut op: F) -> anyhow::Result<()> 
        where F: FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView) -> anyhow::Result<()> {

        let Self {
            device, 
            queue, 
            surface, ..
        } = self;
        
        let output = surface.get_current_texture()?;

        let view = output.texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder({
            &wgpu::CommandEncoderDescriptor::default()
        });

        op(&mut encoder, &view)?;

        queue.submit(Some(encoder.finish()));

        output.present();

        Ok(())
    }
}