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
    pitch: f32,
    yaw: f32,
    eye: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    vertical_fov: f32,
    far_plane: f32,
    dragging: bool,
    scrolling: bool,
}

impl Camera {
    const MULT_DIST: f32 = 1.5;

    const MULT_MIN: f32 = 1.1;
    const MULT_MAX: f32 = 1.666667;

    pub fn new(globe_radius: f32) -> Self {
        let distance = globe_radius * Self::MULT_DIST;

        Self {
            distance,
            globe_radius,
            pitch: 0.,
            yaw: 0.,
            eye: [0., 0., distance * -1.],
            target: [0.; 3],
            up: [0., 1., 0.],
            vertical_fov: std::f32::consts::PI / 2.,
            far_plane: distance * 2.,
            dragging: false,
            scrolling: false,
        }
    }

    pub fn movement_in_progress(&self) -> bool {
        self.dragging || self.scrolling
    }

    pub fn handle_event(&mut self, event: backend::AppEvent) -> bool {
        let mult = ultraviolet::Vec3::from(self.eye).mag().abs() / //
            self.globe_radius;

        let mult = (mult - Self::MULT_MIN) / //
            (Self::MULT_MAX - Self::MULT_MIN) + Self::MULT_MIN - 1.;

        match event {
            backend::AppEvent::Mouse { 
                button: backend::event::MouseButton::Left, 
                state, ..
            } => {
                let temp = self.dragging;

                self.dragging = matches!(
                    state, backend::event::ElementState::Pressed
                );
                
                self.dragging != temp
            },
            backend::AppEvent::MouseScroll { delta } => {
                let lower = delta < 0. && mult > 0.0;
                let upper = delta > 0. && mult < 1.0;

                if lower || upper {
                    let mult = std::f32::consts::E.powf(mult);
                    let mult = mult * self.globe_radius * 0.01;

                    self.distance += delta * mult;

                    self.scrolling = true;

                    true
                } else {
                    false
                }
            },
            backend::AppEvent::MouseScrollStopped => {
                self.scrolling = false;

                true
            },
            backend::AppEvent::MouseMotion { x, y } => {
                if self.dragging {
                    let mult = (((mult + 1.).ln()) * 0.0015).abs();

                    self.pitch -= y * mult;
                    self.pitch = self.pitch.clamp(
                        -1.0 * std::f32::consts::PI / 2. + f32::EPSILON, 
                        std::f32::consts::PI / 2. - f32::EPSILON,
                    );

                    self.yaw -= x * mult;
                }

                self.dragging
            },
            _ => false,
        }
    }

    pub fn build_camera_uniform(&self, screen_resolution: backend::Size) -> CameraUniform {
        let Self {
            eye,
            target,
            up, 
            vertical_fov: fovy,
            far_plane: zfar, ..
        } = self;

        let view = ultraviolet::Mat4::look_at(
            ultraviolet::Vec3::from(eye),
            ultraviolet::Vec3::from(target),
            ultraviolet::Vec3::from(up),
        );

        let backend::Size { width, height } = screen_resolution;

        let proj = ultraviolet::projection::rh_ydown::perspective_gl(
            *fovy,
            width as f32 / height as f32,
            0.1,
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