use std::sync::Arc;

fn configure_surface_resolution(
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
    pub window: Arc<winit::window::Window>,
    pub required_limits: wgpu::Limits,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub surface: wgpu::Surface<'a>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub cursor: crate::Position,
    pub scroll_state: Option<chrono::DateTime<chrono::Local>>,
}

impl<'a> State<'a> {
    // time to wait after a scroll event before sending 
    // crate::AppEvent::MouseScrollStopped
    const SCROLL_THRESHOLD: f32 = 200.;

    pub async fn new(
        event_loop: &winit::event_loop::EventLoop<crate::RequestInternal>,
        window_title: &str,
        surface_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        #[allow(non_snake_case)]
        let LIMITS = wgpu::Limits::downlevel_webgl2_defaults();

        let window = Arc::new({
            winit::window::WindowBuilder::new().with_title(window_title)
                .build(event_loop)?
        });

        // set the window title on wasm32
        #[cfg(target_arch = "wasm32")] 
        web_sys::window()
            .ok_or(wgpu::rwh::HandleError::Unavailable)?
            .document()
            .ok_or(wgpu::rwh::HandleError::Unavailable)?
            .set_title(window_title);

        fn create_surface_target<'a>(
            #[allow(unused_variables)] window: Arc<winit::window::Window>,
        ) -> Result<wgpu::SurfaceTarget<'a>, wgpu::rwh::HandleError> {
            #[cfg(target_arch="wasm32")] {
                use wasm_bindgen::JsCast as _;

                fn create_canvas_handle<'a>(
                    #[allow(unused_variables)] 
                    window: Arc<winit::window::Window>,
                ) -> Result<web_sys::Element, wasm_bindgen::JsValue> {
                    use winit::platform::web::WindowExtWebSys as _;
    
                    let document = web_sys::window()
                        .ok_or(wasm_bindgen::JsValue::UNDEFINED)?
                        .document()
                        .ok_or(wasm_bindgen::JsValue::UNDEFINED)?;
    
                    let elem: web_sys::Element = window
                        .as_ref()
                        .canvas()
                        .ok_or(wasm_bindgen::JsValue::UNDEFINED)?
                        .into();
    
                    // Insert the canvas into the body
                    document.body()
                        .ok_or(wasm_bindgen::JsValue::UNDEFINED)?
                        .append_child(&elem.clone().into())?;

                    Ok(elem)
                }

                let handle = create_canvas_handle(window)
                    .map_err(|_| wgpu::rwh::HandleError::Unavailable)?
                    .dyn_into::<web_sys::HtmlCanvasElement>()
                    .map_err(|_| wgpu::rwh::HandleError::Unavailable)?;

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
            create_surface_target(Arc::clone(&window))?
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

        configure_surface_resolution(
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
            cursor: crate::Position::default(),
            scroll_state: None,
        })
    }

    pub fn run(
        &mut self, 
        event: winit::event::Event<crate::RequestInternal>,
        event_target: &winit::event_loop::EventLoopWindowTarget<crate::RequestInternal>,
    ) -> anyhow::Result<Vec<crate::AppEvent<'static>>> {
        use winit::keyboard::{Key, NamedKey};

        let mut curr = Vec::with_capacity(2);

        match event {
            winit::event::Event::WindowEvent { 
                window_id, 
                event: winit::event::WindowEvent::CloseRequested | //
                    winit::event::WindowEvent::KeyboardInput {
                    event: winit::event::KeyEvent {
                        state: winit::event::ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Escape), ..
                    }, ..
                }
            } if window_id == self.window.id() => event_target.exit(),
            winit::event::Event::WindowEvent { 
                event: winit::event::WindowEvent::Resized(physical_size), 
                window_id, .. 
            } if window_id == self.window.id() => {
                let Self {
                    window,
                    required_limits,
                    device,
                    surface_config, 
                    surface, ..
                } = self;

                configure_surface_resolution(
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

                curr.push(crate::AppEvent::Resized(size));
            },
            winit::event::Event::WindowEvent { 
                event: winit::event::WindowEvent::CursorMoved { position, .. }, 
                window_id, .. 
            } if window_id == self.window.id() => {
                self.cursor = crate::Position::from(position.cast());
            },
            winit::event::Event::WindowEvent { 
                event: winit::event::WindowEvent::KeyboardInput { 
                    event: winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(code),
                        state, ..
                    }, .. 
                }, window_id, .. 
            } if window_id == self.window.id() => {
                curr.push(crate::AppEvent::Key { code, state });
            },
            winit::event::Event::WindowEvent { 
                event: winit::event::WindowEvent::MouseInput { 
                    button, 
                    state, .. 
                }, window_id, .. 
            } if window_id == self.window.id() => {
                curr.push(crate::AppEvent::Mouse { button, state, cursor: self.cursor });
            },
            winit::event::Event::DeviceEvent {
                event: winit::event::DeviceEvent::MouseMotion { 
                    delta: (x, y),
                }, ..
            } => {
                curr.push(crate::AppEvent::MouseMotion { x: x as f32, y: y as f32 });
            },
            winit::event::Event::DeviceEvent {
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

                curr.push(crate::AppEvent::MouseScroll { delta, cursor: self.cursor });

                event_target.set_control_flow({
                    winit::event_loop::ControlFlow::Poll
                });

                self.scroll_state = Some(chrono::Local::now());
            },
            _ => { /*  */ },
        }

        if !matches!(curr.first(), Some(crate::AppEvent::MouseScroll { .. })) {
            if let Some(timestamp) = self.scroll_state.as_ref() {
                let temp = chrono::Local::now();
    
                let duration = timestamp
                    .signed_duration_since(temp)
                    .abs()
                    .num_milliseconds() as f32;
    
                if duration > Self::SCROLL_THRESHOLD {
                    curr.push(crate::AppEvent::MouseScrollStopped);
                }
            }
        }

        if let Some(crate::AppEvent::MouseScrollStopped) = curr.last() {
            self.scroll_state = None;

            event_target.set_control_flow({
                winit::event_loop::ControlFlow::Wait
            });
        }

        Ok(curr)
    }

    pub fn process_encoder<E, F>(&self, mut op: F) -> anyhow::Result<()> where 
        E: Into<anyhow::Error> + Send + Sync, 
        F: FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView) -> Result<(), E> {

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

        op(&mut encoder, &view)
            .map_err(Into::<anyhow::Error>::into)?;

        queue.submit(Some(encoder.finish()));

        output.present();

        Ok(())
    }
}