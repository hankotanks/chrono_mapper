#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Debug)]
pub struct CameraUniform {
    pub eye: [f32; 4],
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
}

#[derive(Clone, Copy)]
pub struct Camera {
    distance: f32,
    globe_radius: f32,
    scale: f32,
    pitch: f32,
    yaw: f32,
    eye: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
    locked: bool,
}

impl Camera {
    pub fn new(globe_radius: f32) -> Self {
        const DISTANCE_MULT: f32 = 1.5;

        Self {
            distance: globe_radius * DISTANCE_MULT,
            globe_radius,
            scale: 1.,
            pitch: 0.,
            yaw: 0.,
            eye: [0., 0., globe_radius * DISTANCE_MULT * -1.],
            target: [0.; 3],
            up: [0., 1., 0.],
            aspect: 1.,
            fovy: std::f32::consts::PI / 2.,
            znear: 0.1,
            zfar: globe_radius * DISTANCE_MULT * 2.,
            locked: true,
        }
    }

    pub fn movement_in_progress(&self) -> bool {
        !self.locked
    }

    pub fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool {
        let mult = ultraviolet::Vec3::from(self.eye).mag().abs() / //
            self.globe_radius;

        let mult_min = 1. + self.znear;
        let mult_max = 1. + (2. / 3.);

        let mult = (mult - mult_min) / (mult_max - mult_min) + mult_min - 1.;

        match event {
            winit::event::DeviceEvent::Button { button: 0, state, } => {
                let temp = self.locked;

                self.locked = matches!(
                    state, 
                    winit::event::ElementState::Released
                );
                
                self.locked != temp
            }
            winit::event::DeviceEvent::MouseWheel { delta, .. } => {
                let scroll_amount = -match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, scroll) => //
                        scroll * 1.0,
                    winit::event::MouseScrollDelta::PixelDelta(
                        winit::dpi::PhysicalPosition { y: scroll, .. }
                    ) => {
                        (self.scale * scroll as f32) / 270.
                    }
                };

                let lower = scroll_amount < 0. && mult > 0.0;
                let upper = scroll_amount > 0. && mult < 1.0;

                if lower || upper {
                    let mult = std::f32::consts::E.powf(mult);
                    let mult = mult * self.globe_radius * 0.01;

                    self.distance += scroll_amount * mult;

                    true
                } else {
                    false
                }
            },
            winit::event::DeviceEvent::MouseMotion { delta: (x, y) } => {
                if !self.locked {
                    let mult = (((mult + 1.).ln()) * 0.0015).abs();

                    self.pitch -= y as f32 * mult;
                    self.pitch = self.pitch.clamp(
                        -1.0 * std::f32::consts::PI / 2. + f32::EPSILON, 
                        std::f32::consts::PI / 2. - f32::EPSILON,
                    );

                    self.yaw -= x as f32 * mult;
                }

                !self.locked
            }
            _ => false,
        }
    }

    pub fn handle_resize(
        &mut self, 
        size: winit::dpi::PhysicalSize<u32>,
        scale: f32,
    ) {
        self.aspect = (size.width as f32) / (size.height as f32);

        self.scale = scale;
    }

    pub fn build_camera_uniform(&self) -> CameraUniform {
        let Self {
            eye,
            target,
            up, 
            fovy,
            aspect,
            znear,
            zfar, ..
        } = self;

        let view = ultraviolet::Mat4::look_at(
            ultraviolet::Vec3::from(eye),
            ultraviolet::Vec3::from(target),
            ultraviolet::Vec3::from(up),
        );

        let proj = ultraviolet::projection::rh_ydown::perspective_gl(
            *fovy,
            *aspect,
            *znear,
            *zfar,
        );

        CameraUniform {
            eye: [eye[0], eye[1], eye[2], 1.],
            view: view
                .as_component_array()
                .map(|ultraviolet::Vec4 { x, y, z, w }| [x, y, z, w]),
            proj: proj
                .as_component_array()
                .map(|ultraviolet::Vec4 { x, y, z, w }| [x, y, z, w]),
        }
    }

    pub fn update(&mut self) -> &Self {
        fn calculate_cartesian_eye_position(
            pitch: f32, 
            yaw: f32, 
            distance: f32,
        ) -> ultraviolet::Vec3 {
            ultraviolet::Vec3::new(
                distance * yaw.sin() * pitch.cos(),
                distance * pitch.sin(),
                distance * yaw.cos() * pitch.cos(),
            )
        }

        self.eye = calculate_cartesian_eye_position(
            self.pitch, 
            self.yaw, 
            self.distance
        ).into();

        self
    }
}