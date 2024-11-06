use backend::wgpu as wgpu;

use super::util;

pub struct Geometry<T: bytemuck::Pod + bytemuck::Zeroable, M: Default> {
    #[allow(dead_code)]
    pub vertices: Vec<T>,
    pub vertex_buffer: wgpu::Buffer,
    pub indices: Vec<u32>,
    pub index_buffer: wgpu::Buffer,
    pub metadata: M,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable, M: Default> Geometry<T, M> {
    pub fn empty(device: &wgpu::Device) -> Self {
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
            metadata: M::default(),
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

#[derive(Clone, Copy)]
pub struct BoundingBox {
    pub centroid: [f32; 3],
    pub tl: [f32; 3],
    pub tr: [f32; 3],
    pub bl: [f32; 3],
    pub br: [f32; 3],
}

#[derive(Default)]
pub struct FeatureMetadata {
    pub entries: Vec<geojson::JsonObject>,
    pub colors: Vec<[u8; 3]>,
    pub bounding_boxes: Vec<(BoundingBox, usize)>,
}

impl Geometry<GlobeVertex, ()> {
    pub fn build_globe_geometry(
        device: &wgpu::Device,
        slices: u32,
        stacks: u32,
        globe_radius: f32,
    ) -> Geometry<GlobeVertex, ()> {
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
            metadata: ()
        }
    }
}

impl Geometry<FeatureVertex, FeatureMetadata> {
    pub fn build_feature_geometry(
        device: &wgpu::Device,
        features: &[geojson::Feature],
        slices: u32,
        stacks: u32,
        globe_radius: f32,
    ) -> Result<Self, earcutr::Error> {
        use wgpu::util::DeviceExt as _;

        fn validation(feature: &geojson::Feature) -> Option<TempFeature<'_>> {
            use geojson::JsonValue;

            TempFeature::validate(feature, |metadata| matches!(
                    metadata.get("NAME"), 
                    Some(JsonValue::Null) | Some(JsonValue::String(_))
            ))
        }

        let maxima = {
            use core::f32;

            let a = (f32::consts::PI * 2. / slices as f32).to_degrees();
            let b = (f32::consts::PI * 2. / stacks as f32).to_degrees();

            a.min(b)
        };

        let mut geometry = TempFeatureGeometry::default();

        for feature in features.iter().filter_map(validation) {
            geometry.add_feature(feature, maxima, globe_radius)?;
        }

        let TempFeatureGeometry { 
            vertices, 
            indices,
            feature_metadata: metadata,
        } = geometry;

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
            metadata,
        })
    }
}

struct TempFeature<'a> {
    geometry: &'a geojson::Geometry,
    metadata: &'a geojson::JsonObject,
}

impl<'a> TempFeature<'a> {
    fn validate(
        feature: &'a geojson::Feature,
        predicate: impl Fn(&geojson::JsonObject) -> bool,
    ) -> Option<Self> {
        let geojson::Feature { geometry, properties, .. } = feature;
        
        match geometry {
            Some(geometry) => {
                let metadata = properties.as_ref();
                match metadata.map(|m| (predicate(m), m)) {
                    Some((passed, metadata)) if passed => //
                        Some(Self { geometry, metadata }),
                    _ => None,
                }
            },
            None => None,
        }
        
    }
}

fn validate_triangle(
    data: &mut Vec<f64>, 
    tri: &[usize],
    maxima: f32,
) -> Vec<usize> {
    struct MajorAxis {
        d: ultraviolet::Vec2,
        fst: [usize; 3],
        snd: [usize; 3],
    }

    let [a_idx, b_idx, c_idx] = *tri else { unreachable!(); };

    let a = ultraviolet::Vec2 {
        x: data[a_idx * 2] as f32, 
        y: data[a_idx * 2 + 1] as f32 
    };

    let b = ultraviolet::Vec2 {
        x: data[b_idx * 2] as f32, 
        y: data[b_idx * 2 + 1] as f32
    };

    let c = ultraviolet::Vec2 {
        x: data[c_idx * 2] as f32, 
        y: data[c_idx * 2 + 1] as f32
    };

    let ab = b - a;
    let bc = c - b;
    let ca = a - c;

    let ab_mag = ab.mag().abs();
    let bc_mag = bc.mag().abs();
    let ca_mag = ca.mag().abs();

    let d_idx = data.len() / 2;

    let axis = if ab_mag > bc_mag && ab_mag > maxima {
        if ab_mag > ca_mag {
            Some(MajorAxis {
                d: a + ab * 0.5,
                fst: [a_idx, d_idx, c_idx],
                snd: [b_idx, c_idx, d_idx],
            })
        } else {
            Some(MajorAxis {
                d: c + ca * 0.5,
                fst: [a_idx, b_idx, d_idx],
                snd: [b_idx, c_idx, d_idx],
            })
        }
    } else if bc_mag > ca_mag && bc_mag > maxima {
        Some(MajorAxis {
            d:  b + bc * 0.5,
            fst: [a_idx, b_idx, d_idx],
            snd: [a_idx, d_idx, c_idx],
        })
    } else if ca_mag > maxima {
        Some(MajorAxis {
            d: c + ca * 0.5,
            fst: [a_idx, b_idx, d_idx],
            snd: [b_idx, c_idx, d_idx],
        })
    } else {
        None
    };

    match axis {
        Some(MajorAxis { d, fst, snd }) => {
            data.push(d.x as f64);
            data.push(d.y as f64);

            let mut indices = Vec::with_capacity(6);
            indices.append(&mut validate_triangle(data, &fst, maxima));
            indices.append(&mut validate_triangle(data, &snd, maxima));

            indices
        },
        None => tri.to_vec(),
    }
}

#[derive(Default)]
struct TempFeatureGeometry {
    vertices: Vec<FeatureVertex>,
    indices: Vec<u32>,
    feature_metadata: FeatureMetadata,
}

impl TempFeatureGeometry {
    fn add_feature(
        &mut self,
        feature: TempFeature<'_>,
        maxima: f32,
        globe_radius: f32,
    ) -> Result<(), earcutr::Error> {
        use core::f32;

        let Self { 
            vertices, 
            indices,
            feature_metadata: FeatureMetadata {
                entries,
                colors,
                bounding_boxes,
            },
        } = self;

        let TempFeature {
            geometry: geojson::Geometry { value, .. },
            metadata, 
        } = feature;

        let color_raw = util::hashable_to_rgb8(metadata);

        let color = [
            color_raw[0] as f32 / 255.,
            color_raw[1] as f32 / 255.,
            color_raw[2] as f32 / 255.,
        ];

        let globe_radius = globe_radius + 1.;

        if let geojson::Value::MultiPolygon(multi_polygon) = value {
            let idx = entries.len();
            
            entries.push(metadata.clone());
            colors.push(color_raw);

            for polygon in multi_polygon {
                let mut bb_min = [f32::MAX; 2];
                let mut bb_max = [f32::MIN; 2];

                let (mut data, holes, dims) = earcutr::flatten(polygon);

                let vertices_len = vertices.len();

                let polygon_indices: Vec<u32> = earcutr::earcut(&data, &holes, dims)?
                    .chunks_exact(3)
                    .flat_map(|tri| validate_triangle(&mut data, tri, maxima))
                    .map(|idx| (idx + vertices_len) as u32)
                    .collect();

                indices.extend_from_slice(&polygon_indices);

                vertices.extend(data.chunks_exact(2).map(|pt| {
                    let pt = [pt[1] as f32, pt[0] as f32];

                    bb_min[0] = bb_min[0].min(pt[0]);
                    bb_min[1] = bb_min[1].min(pt[1]);

                    bb_max[0] = bb_max[0].max(pt[0]);
                    bb_max[1] = bb_max[1].max(pt[1]);

                    let pos = util::lat_lon_to_vertex(pt, globe_radius);

                    FeatureVertex { pos, color }
                }));

                let mut centroid_sum = 0.;
                let mut centroid_accum = ultraviolet::Vec3::zero();

                for tri in polygon_indices.chunks_exact(3) {
                    let [a_idx, b_idx, c_idx] = *tri else { unreachable!(); };

                    if (holes.contains(&(a_idx as usize - vertices_len))) || //
                        holes.contains(&(b_idx as usize - vertices_len)) || //
                        holes.contains(&(c_idx as usize - vertices_len)) { continue; }

                    let a = ultraviolet::Vec3::from(vertices[a_idx as usize].pos);
                    let b = ultraviolet::Vec3::from(vertices[b_idx as usize].pos);
                    let c = ultraviolet::Vec3::from(vertices[c_idx as usize].pos);

                    let tri_centroid = (a + b + c) / 3.;

                    let tri_area = (b - a).cross(c - a).mag();       

                    centroid_sum += tri_area;
                    centroid_accum += tri_centroid * tri_area;         
                }

                let tl = util::lat_lon_to_vertex(bb_min, globe_radius);
                let tr = util::lat_lon_to_vertex([bb_max[0], bb_min[1]], globe_radius);
                let bl = util::lat_lon_to_vertex([bb_min[0], bb_max[1]], globe_radius);
                let br = util::lat_lon_to_vertex(bb_max, globe_radius);

                let centroid = centroid_accum / centroid_sum;

                let bb = BoundingBox {
                    centroid: *centroid.as_array(), tl, tr, bl, br,
                };

                bounding_boxes.push((bb, idx));
            }  
        }

        Ok(())
    }
}