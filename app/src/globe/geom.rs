use std::hash;

pub struct Geometry<T: bytemuck::Pod + bytemuck::Zeroable> {
    #[allow(dead_code)]
    pub vertices: Vec<T>,
    pub vertex_buffer: wgpu::Buffer,
    pub indices: Vec<u32>,
    pub index_buffer: wgpu::Buffer,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> Geometry<T> {
    pub fn empty(device: &wgpu::Device) -> Geometry<T> {
        use std::mem;

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<T>() as u64,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
    
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4u64,
            usage: wgpu::BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        Self {
            vertices: Vec::with_capacity(0),
            vertex_buffer,
            indices: Vec::with_capacity(0),
            index_buffer,
        }
    }

    pub fn destroy(self) {
        let Self { vertex_buffer, index_buffer, .. } = self;

        vertex_buffer.destroy();

        index_buffer.destroy();
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobeVertex { pub pos: [f32; 3] }

impl GlobeVertex {
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
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: Self::VERTEX_ATTRIBUTES,
        }
    }
}

impl Geometry<GlobeVertex> {
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
}

fn hashable_to_rgba8(name: impl hash::Hash) -> [u8; 4] {
    use hash::Hasher as _;

    let mut hasher = hash::DefaultHasher::new();

    name.hash(&mut hasher);

    let hashed = hasher.finish();

    [
        ((hashed & 0xFF0000) >> 16) as u8,
        ((hashed & 0x00FF00) >> 8) as u8,
        (hashed & 0x0000FF) as u8,
        255u8,
    ]
}

struct TempFeature<'a> {
    name: &'a str,
    geometry: &'a geojson::Geometry,
}

impl<'a> TempFeature<'a> {
    fn validate_feature_properties(feature: &'a geojson::Feature) -> Option<Self> {
        let geojson::Feature { geometry, properties, .. } = feature;

        match properties {
            Some(properties)  => match properties.get("NAME") {
                Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(name)) => {
                    geometry.as_ref().map(|geometry| {
                        TempFeature { name, geometry }
                    })
                }, _ => None,
            }, _ => None,
        }
    }
}

type TempPoint = (f64, f64);

type TempTri = (usize, usize, usize);

#[derive(Default)]
struct TempPolygon {
    points: Vec<(f64, f64)>,
    contours: Vec<Vec<usize>>,
}

impl TempPolygon {
    fn add_linear_ring(&mut self, lr: &[Vec<f64>]) -> Result<(), ()> {
        if lr.len() < 5 { Err(())?; }

        let offset = self.points.len();
        let mut curr = (offset..(lr.len() + offset))
            .collect::<Vec<_>>();
        
        curr[lr.len() - 1] = curr[0];

        self.contours.push(curr);
        self.points.extend({
            lr[0..(lr.len() - 1)].iter().map(|v| (v[1], v[0]))
        });

        Ok(())
    }

    fn triangulate(self) -> Result<(Vec<TempPoint>, Vec<TempTri>), cdt::Error> {
        let Self { points, contours } = self;

        cdt::triangulate_contours(&points, &contours)
            .map(|triangles| (points, triangles))
    }
}

struct TempFeatureGeometry {
    vertices: Vec<FeatureVertex>,
    indices: Vec<u32>,
}

impl TempFeatureGeometry {
    fn build_feature_geometry_raw(
        features: &[geojson::Feature],
        globe_radius: f32,
    ) -> Self {
        let mut vertices = Vec::new();
        
        let mut indices = Vec::new();
    
        for feature in features.iter().filter_map(TempFeature::validate_feature_properties) {
            let TempFeature { 
                name, 
                geometry: geojson::Geometry { value, .. },
            } = feature;
    
            let color = hashable_to_rgba8(name);
            let color = [
                color[0] as f32 / 255.,
                color[1] as f32 / 255.,
                color[2] as f32 / 255.,
            ];
    
            if let geojson::Value::MultiPolygon(polygons) = value {
                'raw: for polygon_raw in polygons {
                    let mut polygon = TempPolygon::default();
    
                    'lr: for (idx, lr) in polygon_raw.iter().enumerate() {
                        match polygon.add_linear_ring(lr) {
                            Ok(_) => { /*  */ },
                            Err(_) if idx == 0usize => {
                                log::info!("skipping {name} [Error: \"invalid outer contour\"]");
                                continue 'raw;
                            },
                            Err(_) => continue 'lr,
                        }
                    }
    
                    match polygon.triangulate() {
                        Ok((points, triangles)) => {
                            let offset = vertices.len();
    
                            vertices.extend(points.into_iter().map(|(x, y)| {
                                use core::f32;
            
                                let lat = x.to_radians() as f32 + f32::consts::PI;
                                let lon = y.to_radians() as f32;
                        
                                FeatureVertex {
                                    pos: [
                                        lat.cos() * lon.cos() * (globe_radius + 1.) * -1., 
                                        lat.sin() * (globe_radius + 1.),
                                        lat.cos() * lon.sin() * (globe_radius + 1.),
                                    ], color,
                                }
                            }));
    
                            for (a, b, c) in triangles.into_iter() {
                                indices.push((a + offset) as u32);
                                indices.push((b + offset) as u32);
                                indices.push((c + offset) as u32);
                            }
                        },
                        Err(e) => {
                            log::info!("skipping {name} [{e}]"); continue 'raw;
                        },
                    }
                }
            }
        }

        Self { vertices, indices }
    }
}

impl Geometry<FeatureVertex> {
    pub fn build_feature_geometry(
        device: &wgpu::Device,
        features: &[geojson::Feature],
        globe_radius: f32,
    ) -> Result<Geometry<FeatureVertex>, cdt::Error> {
        use wgpu::util::DeviceExt as _;
    
        let TempFeatureGeometry {
            vertices,
            indices,
        } = TempFeatureGeometry::build_feature_geometry_raw(
            features, 
            globe_radius,
        );
    
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
    
        Ok(Geometry {
            vertices,
            vertex_buffer,
            indices,
            index_buffer,
        })
    }
}