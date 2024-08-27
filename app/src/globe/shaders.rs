use std::{collections, io, str};

pub(super) fn load_shader<'a>(
    name: &str,
    assets: &collections::HashMap<&'a str, &'a [u8]>,
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