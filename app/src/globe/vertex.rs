use std::mem;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex { pub pos: [f32; 3] }

impl Vertex {
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute] = &{
        wgpu::vertex_attr_array![0 => Float32x3]
    };

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::VERTEX_ATTRIBUTES,
        }
    }
}

pub struct Geometry {
    #[allow(dead_code)] pub vertices: Vec<Vertex>,
    pub vertex_buffer: wgpu::Buffer,
    #[allow(dead_code)] pub indices: Vec<u32>,
    pub index_count: u32,
    pub index_buffer: wgpu::Buffer,
}

impl Geometry {
    pub fn generate(
        device: &wgpu::Device,
        slices: u32,
        stacks: u32,
        globe_radius: f32,
    ) -> Self {
        use wgpu::util::DeviceExt as _;

        let mut vertices = vec![Vertex { 
            pos: [0., globe_radius, 0.] 
        }];

        for i in 0..(stacks - 1) {
            let phi = (std::f32::consts::PI * (i + 1) as f32) / //
                (stacks as f32);

            for j in 0..slices {
                let theta = (std::f32::consts::PI * 2. * j as f32) / // 
                    (slices as f32);

                vertices.push(Vertex { 
                    pos: [
                        phi.sin() * theta.cos() * globe_radius,
                        phi.cos() * globe_radius,
                        phi.sin() * theta.sin() * globe_radius,
                    ],
                });
            }
        }

        vertices.push(Vertex { 
            pos: [0., globe_radius * -1., 0.] 
        });

        let v0 = 0;
        let v1 = vertices.len() as u32 - 1;

        let mut indices = Vec::with_capacity(vertices.len());

        for i in 0..slices {
            let i0 = i + 1;
            let i1 = (i0 % slices) + 1;
            indices.extend([v0, i1, i0]);

            let i0 = i + slices * (stacks - 2) + 1;
            let i1 = (i + 1) % slices + slices * (stacks - 2) + 1;
            indices.extend([v1, i0, i1]);
        }

        for j in 0..(stacks - 2) {
            let j0 = j * slices + 1;
            let j1 = (j + 1) * slices + 1;

            for i in 0..slices {
                let i0 = j0 + i;
                let i1 = j0 + (i + 1) % slices;
                let i2 = j1 + (i + 1) % slices;
                let i3 = j1 + i;

                indices.extend([i3, i0, i1, i1, i2, i3]);
            }
        }

        let vertex_buffer = device.create_buffer_init(&{
            wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        });

        let index_count = indices.len() as u32;

        let index_buffer = device.create_buffer_init(&{
            wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }
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