use std::mem;

pub struct Geometry<T: bytemuck::Pod + bytemuck::Zeroable> {
    #[allow(dead_code)]
    pub vertices: Vec<T>,
    pub vertex_buffer: wgpu::Buffer,
    pub indices: Vec<u32>,
    pub index_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobeVertex { pub pos: [f32; 3] }

impl GlobeVertex {
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FeatureVertex { 
    pub pos: [f32; 3],
    pub color: [f32; 3],
}

impl FeatureVertex {
    const VERTEX_ATTRIBUTES: &'static [wgpu::VertexAttribute] = &{
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3]
    };

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::VERTEX_ATTRIBUTES,
        }
    }
}

pub fn build_globe_geometry(
    device: &wgpu::Device,
    slices: u32,
    stacks: u32,
    globe_radius: f32,
) -> Geometry<GlobeVertex> {
    use wgpu::util::DeviceExt as _;

    let mut vertices = vec![GlobeVertex { 
        pos: [0., globe_radius, 0.] 
    }];

    for i in 0..(stacks - 1) {
        let phi = (std::f32::consts::PI * (i + 1) as f32) / //
            (stacks as f32);

        for j in 0..slices {
            let theta = (std::f32::consts::PI * 2. * j as f32) / // 
                (slices as f32);

            vertices.push(GlobeVertex { 
                pos: [
                    phi.sin() * theta.cos() * globe_radius,
                    phi.cos() * globe_radius,
                    phi.sin() * theta.sin() * globe_radius,
                ],
            });
        }
    }

    vertices.push(GlobeVertex { 
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

    let index_buffer = device.create_buffer_init(&{
        wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        }
    });

    Geometry {
        vertices,
        vertex_buffer,
        indices,
        index_buffer,
    }
}

pub fn build_feature_geometry(
    device: &wgpu::Device,
    features: &[geojson::Feature],
    globe_radius: f32,
) -> Geometry<FeatureVertex> {
    use wgpu::util::DeviceExt as _;

    let mut vertices = Vec::new();

    let mut indices = Vec::new();

    fn validate_feature(f: &geojson::Feature) -> Option<&geojson::Geometry> {
        let geojson::Feature { geometry, properties, .. } = f;

        match properties {
            Some(properties) if properties.contains_key("NAME") => {
                geometry.as_ref()
            }, _ => None,
        }
    }

    for geometry in features.iter().filter_map(validate_feature) {
        let geojson::Geometry { value, .. } = geometry;

        if let geojson::Value::MultiPolygon(polygons) = value {
            for polygon in polygons {
                if let Some(outer) = polygon.first() {
                    let points = outer
                        .iter()
                        .map(|vertex| delaunator::Point {
                            x: vertex[0], y: vertex[1],
                        }).collect::<Vec<_>>();

                    let offset = vertices.len();
                    indices.extend({
                        delaunator::triangulate(points.as_slice())
                            .triangles
                            .into_iter()
                            .map(|index| (index + offset) as u32)
                    });

                    let color = random_color::RandomColor::new()
                        .to_rgb_array();

                    let color = [
                        color[0] as f32 / 255., 
                        color[1] as f32 / 255., 
                        color[2] as f32 / 255.,
                    ];

                    vertices.extend({
                        points
                            .into_iter()
                            .map(|delaunator::Point { x, y }| {
                                use core::f32;

                                let lat = (x as f32 / 180.) * f32::consts::PI;
                                let lon = (y as f32 / 180.) * f32::consts::PI;

                                FeatureVertex {
                                    pos: [
                                        lat.cos() * lon.cos() * globe_radius, 
                                        lat.cos() * lon.sin() * globe_radius, 
                                        lat.sin() * globe_radius,
                                    ], color,
                                }
                            })
                    });
                }
            }
        }
    }

    let vertex_buffer = device.create_buffer_init(&{
        wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }
    });

    let index_buffer = device.create_buffer_init(&{
        wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        }
    });

    Geometry {
        vertices,
        vertex_buffer,
        indices,
        index_buffer,
    }
}