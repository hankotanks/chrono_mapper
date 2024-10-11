use std::{env, fs, io, path};

fn get_env_var(var: &'static str) -> io::Result<String> {    
    env::var(var).map_err(|e| match e {
        env::VarError::NotPresent => io::Error::new(io::ErrorKind::NotFound, format!("Environment variable `{}` undefined in this scope.", var)),
        env::VarError::NotUnicode(os_string) => io::Error::new(io::ErrorKind::InvalidInput, format!("Environment variable `{}` contained non-UTF8 characters: `{}`", var, os_string.to_string_lossy())),
    })
}

fn copy_dir_into(
    src: impl AsRef<path::Path>, 
    dst: impl AsRef<path::Path>,
) -> io::Result<()> {
    fs::create_dir_all(&dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;

        if entry.file_type()?.is_dir() {
            copy_dir_into(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }; Ok(())
}

fn main() -> io::Result<()> {
    let assets_path = match env::var("BACKEND_STATIC_ASSETS_DIR") {
        Ok(assets_path) => Ok(assets_path),
        Err(env::VarError::NotPresent) => {
            let root = path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .ok_or(io::Error::new(io::ErrorKind::NotFound, format!("Directory defined by `CARGO_MANIFEST_DIR` has no parent: `{}`.", env!("CARGO_MANIFEST_DIR"))))?
                .join("assets");

            match root.exists() {
                true => root.to_str().map(String::from).ok_or(io::Error::new(io::ErrorKind::InvalidData, format!("Unable to access parent directory of `CARGO_MANIFEST_DIR` because it contains non-UTF8 characters: `{}`", env!("CARGO_MANIFEST_DIR")))),
                false => Err(io::Error::new(io::ErrorKind::NotFound, format!("Directory defined by `CARGO_MANIFEST_DIR` has no parent: `{}`.", env!("CARGO_MANIFEST_DIR")))),
            }
        },
        Err(env::VarError::NotUnicode(os_string)) => {
            let e = match os_string.into_string() {
                Ok(assets_path_invalid) => io::Error::new(io::ErrorKind::InvalidData, assets_path_invalid), 
                Err(_) => io::Error::from(io::ErrorKind::InvalidData),
            }; Err(e)
        },
    }?;

    static_files::resource_dir(assets_path).build()?;

    let root = get_env_var("CARGO_MANIFEST_DIR")?;
    let root = path::Path::new(&root);

    let out = match get_env_var("BACKEND_OUT_DIR") {
        Ok(backend_out_dir) => backend_out_dir,
        Err(_) => {
            let out = get_env_var("OUT_DIR")?;
            println!("cargo:rustc-env=BACKEND_OUT_DIR={out}");
            out
        },
    }; let out = path::Path::new(&out);

    copy_dir_into(root.join("js"), &out)?;
    copy_dir_into(root.join("static"), &out)?;

    if let Ok(local_dir_var) = get_env_var("BACKEND_LOCAL_ASSETS_DIR") {
        let local_dir = path::Path::new(&local_dir_var);
        let local_dir_name = local_dir
            .file_name()
            .ok_or(io::Error::new(io::ErrorKind::InvalidData, format!("Failed to find the local assets directory (or it terminated in ..): `{}`", local_dir_var)))?;
        let local_dir_name = local_dir_name
            .to_str()
            .ok_or(io::Error::new(io::ErrorKind::InvalidData, format!("The path specified by `BACKEND_LOCAL_ASSETS_DIR` contained non-UTF8 symbols: {}", local_dir_name.to_string_lossy())))?;
    
        copy_dir_into(local_dir, out.join(local_dir_name))?;
    }

    Ok(())
}