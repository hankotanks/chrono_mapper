use std::{hash, io};

type ShaderResult<'a> = Result<wgpu::ShaderModuleDescriptor<'a>, io::Error>;

pub fn load_shader<'a>(aref: backend::AssetRef<'a>) -> ShaderResult<'a> {
    fn load_shader_inner<'a>(aref: backend::AssetRef<'a>) -> Result<String, io::Error> {
        if let Some(source) = backend::Assets::retrieve(aref) {
            let source = String::from_utf8(source.to_vec())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;

            let mut source_full = String::new();
            for includes in source.lines() {
                if includes.contains("include") {
                    if let Some(path) = includes.split_whitespace().nth(1) {
                        let aref_sub = backend::AssetRef {
                            path,
                            locator: aref.locator,
                        };

                        source_full.push_str(&load_shader_inner(aref_sub)?);
                    }
                } else {
                    break;
                }
            }

            source_full.push_str(&source);

            Ok(source_full)
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

	Ok(wgpu::ShaderModuleDescriptor {
		label: None,
		source: wgpu::ShaderSource::Wgsl(load_shader_inner(aref)?.into()),
	})
}

#[allow(dead_code)]
pub fn load_features_from_geojson<'a>(
    aref: backend::AssetRef<'a>,
) -> anyhow::Result<Vec<geojson::Feature>> {
    use std::str;
    
    let data = backend::Assets::retrieve(aref)
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

    let features = str::from_utf8(data)?.parse::<geojson::GeoJson>()?;

    let collection = geojson::FeatureCollection::try_from(features)?.features;

    Ok(collection)
}

pub fn cursor_to_world_ray(
    view: [[f32; 4]; 4], 
    proj: [[f32; 4]; 4], 
    cursor: winit::dpi::PhysicalPosition<f32>,
) -> [f32; 3] {
    use ultraviolet::{Vec2, Vec4, Mat4};

    let winit::dpi::PhysicalPosition { x, y } = cursor;

    let Vec2 { 
        x, y,
    } = (Mat4::from(proj).inversed() * Vec4::new(x, y, -1., 1.)).xy();

    *(Mat4::from(view).inversed() * Vec4::new(x, y, -1., 0.))
        .xyz().normalized().as_array()
}

pub fn world_to_screen_space(
    vertex: [f32; 3],
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
) -> [f32; 2] {
    use ultraviolet::{Vec3, Vec4, Mat4};

    let mut v = Vec4::from(Vec3::from(vertex));
    
    v.w = 1.;

    let v = Mat4::from(proj) * Mat4::from(view) * v;

    *(v / v.w).xy().as_array()
}

pub fn intrs(
    eye: [f32; 4],
    ray: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    maxima_sq: f32,
) -> f32 {
    use core::f32;

    const EPS: f32 = 0.0000001;

    let eye = ultraviolet::Vec4::from(eye).xyz();

    let a = ultraviolet::Vec3::from(a);
    let b = ultraviolet::Vec3::from(b);
    let c = ultraviolet::Vec3::from(c);

    let e1 = b - a;
    let e2 = c - a;

    let ray = ultraviolet::Vec3::from(ray);

    let p = ray.cross(e2);
    let t = eye - a;
    let q = t.cross(e1);

    let det = e1.dot(p);

    let u = t.dot(p);
    let v = ray.dot(q);

    #[allow(clippy::if_same_then_else)]
    if det > EPS && (u < 0. || u > det) {
        f32::MAX
    } else if det > EPS && (v < 0. || u + v > det) {
        f32::MAX
    } else if det < EPS * -1. {
        f32::MAX
    } else {
        let w = e2.dot(q) / det;

        match w * w > maxima_sq || w < 0.1 { 
            true => f32::MAX,
            false => (ray * w).mag(),
        }
    }
}

pub fn hemisphere_maxima_sq(
    eye: [f32; 4],
    globe_radius: f32,
) -> f32 {
    let a = ultraviolet::Vec4::from(eye).xyz().mag();
    let b = globe_radius * globe_radius;

    a * a  + b * b
}

pub fn lat_lon_to_vertex(pt: [f32; 2], globe_radius: f32) -> [f32; 3] {
    use core::f32;

    let lat = pt[0].to_radians() + f32::consts::PI;
    let lon = pt[1].to_radians();

    [
        lat.cos() * lon.cos() * globe_radius * -1., 
        lat.sin() * globe_radius,
        lat.cos() * lon.sin() * globe_radius,
    ]
}

#[allow(unused_parens, clippy::double_parens)]
pub fn hashable_to_rgb8(name: &(impl hash::Hash)) -> [u8; 3] {
    use hash::Hasher as _;

    let mut hasher = hash::DefaultHasher::new();

    name.hash(&mut hasher);

    let color_hash = hasher.finish();
    let color = [
        ((color_hash & 0xFF0000) >> 16) as u8,
        ((color_hash & 0x00FF00) >> 8) as u8,
        ((color_hash & 0x0000FF)) as u8,
    ];

    let diff = 255u8 - color.into_iter().fold(0, |m, c| m.max(c));

    [
        color[0] + diff, 
        color[1] + diff, 
        color[2] + diff,
    ]
}