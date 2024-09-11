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
    name: &'a str,
) -> anyhow::Result<Vec<geojson::Feature>> {
    use std::str;
    
    let data = assets
        .get(name)
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

    let features = str::from_utf8(data)?.parse::<geojson::GeoJson>()?;

    let collection = geojson::FeatureCollection::try_from(features)?.features;

    Ok(collection)
}

pub fn str_to_rgba8(name: &str) -> [u8; 4] {
    use std::hash;
    use std::hash::Hasher as _;
    use std::hash::Hash as _;

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

pub fn validate_feature_properties(
    feature: &geojson::Feature,
) -> Option<(&str, &geojson::Geometry)> {
    let geojson::Feature { geometry, properties, .. } = feature;

    match properties {
        Some(properties)  => match properties.get("NAME") {
            Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(name)) => {
                geometry.as_ref().map(|g| (name.as_str(), g))
            }, _ => None,
        }, _ => None,
    }
}
