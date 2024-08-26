use std::mem;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] = //
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::VERTEX_ATTRIBUTES,
        }
    }
}

pub struct Geometry {
    #[allow(dead_code)]
    pub vertices: Vec<Vertex>,
    pub vertex_buffer: wgpu::Buffer,

    #[allow(dead_code)]
    pub indices: Vec<u16>,
    pub index_count: u32,
    pub index_buffer: wgpu::Buffer,
}

impl Geometry {
    pub fn generate(
        _slices: u32,
        _stacks: u32,
        device: &wgpu::Device,
    ) -> Self {
        use wgpu::util::DeviceExt as _;

        let vertices = vec![
            Vertex { position: [-0.0868241, 0.49240386, 0.0], color: [0.5, 0.0, 0.5] },
            Vertex { position: [-0.49513406, 0.06958647, 0.0], color: [0.5, 0.0, 0.5] },
            Vertex { position: [-0.21918549, -0.44939706, 0.0], color: [0.5, 0.0, 0.5] },
            Vertex { position: [0.35966998, -0.3473291, 0.0], color: [0.5, 0.0, 0.5] },
            Vertex { position: [0.44147372, 0.2347359, 0.0], color: [0.5, 0.0, 0.5] },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = vec![
            0, 1, 4,
            1, 2, 4,
            2, 3, 4,
        ];

        let index_count = indices.len() as u32;

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertices,
            vertex_buffer,
            indices,
            index_count,
            index_buffer,
        }
    }
}