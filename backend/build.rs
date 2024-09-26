use std::{fs, io, path};

fn build_assets(root: path::PathBuf) -> io::Result<()> {
    use static_files::resource_dir;
    
    let assets_dir = root.join("assets");

    if !assets_dir.exists() { 
        let assets_dir_str = assets_dir
            .to_str()
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

        fs::create_dir(assets_dir_str)?;
    }

    resource_dir(assets_dir).build()
}

fn set_workspace_root(root: path::PathBuf) -> io::Result<()> {
    let root = root
        .to_str()
        .ok_or(io::Error::from(io::ErrorKind::InvalidData))?;

    println!("cargo:rustc-env=WORKSPACE_ROOT={root}");

    Ok(())
}

fn main() -> io::Result<()> {
    let root = path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(io::Error::from(io::ErrorKind::NotFound))?;

    build_assets(root.to_path_buf())?;
    
    #[cfg(not(target_arch = "wasm32"))]
    set_workspace_root(root.to_path_buf())?;

    Ok(())
}