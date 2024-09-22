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

pub fn cast(
    eye: [f32; 4],
    view: [[f32; 4]; 4], 
    proj: [[f32; 4]; 4], 
    cursor: winit::dpi::PhysicalPosition<f32>,
    globe_radius: f32,
) {
    let winit::dpi::PhysicalPosition { x, y } = cursor;

    let ultraviolet::Vec2 { x, y } = (
        ultraviolet::Mat4::from(proj).inversed() * //
        ultraviolet::Vec4::new(x, y, -1., 1.)
    ).xy();

    let ray_world = (
        ultraviolet::Mat4::from(view).inversed() * //
        ultraviolet::Vec4::new(x, y, -1., 0.)
    ).xyz().normalized();

    println!("{:?}", intersection(ultraviolet::Vec4::from(eye).xyz(), ray_world, globe_radius));
}

pub fn intersection(
    eye: ultraviolet::Vec3,
    dir: ultraviolet::Vec3,
    globe_radius: f32,
) -> bool {
    let a = dir.dot(dir);
    let b = eye.dot(dir) * 2.;
    let c = eye.dot(eye) - globe_radius * globe_radius;

    let disc = b * b - 4.0 * a * c;

    if disc < 0.0 {
        return false;
    } else {
        let b_neg = b * -1.;
        let disc_sqrt = disc.sqrt();
        let t0 = b_neg - disc_sqrt / (a * 2.);
        let t1 = b_neg + disc_sqrt / (a * 2.);

        return t0 > 0.0 || t1 > 0.0;
    }
}