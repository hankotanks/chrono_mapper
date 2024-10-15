use std::io;

use backend::wgpu as wgpu;

type ShaderResult<'a> = io::Result<wgpu::ShaderModuleDescriptor<'a>>;

pub async fn load<'a>(data: &backend::AppData<'a>, path: &str) -> ShaderResult<'a> {
    fn helper<'a>(
        data: &backend::AppData<'_>,
        lines: impl Iterator<Item = &'a str>,
    ) -> io::Result<String> {
        let mut full = String::new();
        
        for line in lines {
            if line.contains("include") {
                if let Some(path) = line.split_whitespace().nth(1) {
                    full.push_str(&(load_inner(data, path))?);
                }
            } else { break; }
        }

        Ok(full)
    }

    fn load_inner(data: &backend::AppData<'_>, path: &str) -> io::Result<String> {
        let source = data.get_static_asset(path)?.to_vec();

        match String::from_utf8(source) {
            Ok(source) => {
                let mut full = helper(data, source.lines())?;

                full.push_str(&source);

                Ok(full)
            },
            Err(_) => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }

	Ok(wgpu::ShaderModuleDescriptor {
		label: None,
		source: wgpu::ShaderSource::Wgsl(load_inner(data, path)?.into()),
	})
}