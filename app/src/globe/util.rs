use std::{collections, io};

pub fn load_shader<'a>(
    assets: &collections::HashMap<&'a str, &'a [u8]>,
    name: &str,
) -> Result<wgpu::ShaderModuleDescriptor<'a>, io::Error> {
    fn as_asset_path(include: &str) -> String {
        let mut path = include.replace("::", "/");

        path.push_str(".wgsl");
        path
    }

    fn load_shader_inner<'a>(
        path: &str, 
        assets: &collections::HashMap<&'a str, &'a [u8]>,
    ) -> Result<String, io::Error> {
        if let Some(source) = assets.get(path) {
            let source = String::from_utf8(source.to_vec())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;

            let mut source_full = String::new();
            for includes in source.lines() {
                if includes.contains("include") {
                    if let Some(module) = includes.split_whitespace().nth(1) {
                        let module = as_asset_path(module);
                        let module = load_shader_inner(&module, assets).unwrap();
    
                        source_full.push_str(&module);
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
		source: wgpu::ShaderSource::Wgsl({
            load_shader_inner(&as_asset_path(name), assets)?.into()
        }),
	})
}

#[allow(dead_code)]
pub fn load_features_from_geojson<'a>(
    assets: &collections::HashMap<&'a str, &'a [u8]>,
    path: &'a str,
) -> anyhow::Result<Vec<geojson::Feature>> {
    use std::str;
    
    let data = assets
        .get(format!("{}.geojson", path.replace("::", "/")).as_str())
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

    let features = str::from_utf8(data)?.parse::<geojson::GeoJson>()?;

    let collection = geojson::FeatureCollection::try_from(features)?.features;

    Ok(collection)
}

pub fn cursor_to_world_ray(
    view: [[f32; 4]; 4], 
    proj: [[f32; 4]; 4], 
    cursor: winit::dpi::PhysicalPosition<f32>,
) -> ultraviolet::Vec3 {
    use ultraviolet::{Vec2, Vec4, Mat4};

    let winit::dpi::PhysicalPosition { x, y } = cursor;

    let Vec2 { 
        x, y,
    } = (Mat4::from(proj).inversed() * Vec4::new(x, y, -1., 1.)).xy();

    (Mat4::from(view).inversed() * Vec4::new(x, y, -1., 0.))
        .xyz().normalized()
}

pub fn _globe_intersection_temp(
    eye: ultraviolet::Vec3,
    dir: ultraviolet::Vec3,
    globe_radius: f32,
) -> bool {
    let a = dir.dot(dir);
    let b = eye.dot(dir) * 2.;
    let c = eye.dot(eye) - globe_radius * globe_radius;

    let disc = b * b - 4.0 * a * c;

    if disc < 0.0 {
        false
    } else {
        let b_neg = b * -1.;
        let disc_sqrt = disc.sqrt();
        let t0 = b_neg - disc_sqrt / (a * 2.);
        let t1 = b_neg + disc_sqrt / (a * 2.);

        t0 > 0.0 || t1 > 0.0
    }
}

pub fn intrs(
    eye: [f32; 4],
    ray: ultraviolet::Vec3,
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

        // TODO: hard-coded near clipping plane value
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