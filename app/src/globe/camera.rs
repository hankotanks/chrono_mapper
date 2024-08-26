#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[derive(Debug)]
pub struct CameraUniform {
    eye: [f32; 4],
    view: [[f32; 4]; 4],
}

#[derive(Clone, Copy)]
pub struct Camera {
    distance: f32,
    pitch: f32,
    yaw: f32,
    eye: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
    speed_rotate: f32,
    speed_zoom: f32,
    locked: bool,
}

impl Camera {
    pub fn new(distance: f32, aspect: f32) -> Self {
        Self {
            distance,
            pitch: 0.,
            yaw: 0.,
            eye: [0., 0., distance * -1.0],
            target: [0.; 3],
            up: [0., 1., 0.],
            aspect,
            fovy: std::f32::consts::PI / 2.,
            znear: 0.1,
            zfar: 1000.,
            speed_rotate: 0.01,
            speed_zoom: 0.6,
            locked: true,
        }
    }

    pub fn handle_event(&mut self, event: winit::event::DeviceEvent) -> bool {
        match event {
            winit::event::DeviceEvent::Button { button: 0, state, } => {
                self.locked = matches!(
                    state, 
                    winit::event::ElementState::Released
                );
                
                false
            }
            winit::event::DeviceEvent::MouseWheel { delta, .. } => {
                let scroll_amount = -match delta {
                    // A mouse line is about 1 px.
                    winit::event::MouseScrollDelta::LineDelta(_, scroll) => //
                        scroll * 1.0,
                    winit::event::MouseScrollDelta::PixelDelta(
                        winit::dpi::PhysicalPosition { y: scroll, .. }
                    ) => scroll as f32,
                };

                self.distance += scroll_amount * self.speed_zoom;

                true
            }
            winit::event::DeviceEvent::MouseMotion { delta: (x, y) } => {
                if !self.locked {
                    self.pitch -= y as f32 * self.speed_rotate;
                    self.pitch = self.pitch.clamp(
                        -1.0 * std::f32::consts::PI / 2. + f32::EPSILON, 
                        std::f32::consts::PI / 2. - f32::EPSILON,
                    );

                    self.yaw -= x as f32 * self.speed_rotate;
                }

                !self.locked
            }
            _ => false,
        }
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

        let proj = ultraviolet::projection::perspective_vk(
            *fovy,
            *aspect,
            *znear,
            *zfar,
        );

        CameraUniform {
            eye: [eye[0], eye[1], eye[2], 1.],
            view: (proj * view)
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