use backend::wgpu as wgpu;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex { pub pos: [f32; 3] }

impl Vertex {
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute] = &{
        wgpu::vertex_attr_array![0 => Float32x3]
    };

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::VERTEX_ATTRIBUTES,
        }
    }
}