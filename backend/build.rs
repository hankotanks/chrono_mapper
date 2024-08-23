use std::{path, fs, io};

use static_files::resource_dir;

fn main() -> std::io::Result<()> {
    let root = path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

    let assets_dir = root.join("assets");

    if !assets_dir.exists() { 
        let assets_dir_str = assets_dir
            .to_str()
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

        fs::create_dir(assets_dir_str)?;
    }

    resource_dir(assets_dir).build()?;

    Ok(())
}