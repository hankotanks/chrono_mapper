mod globe;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub struct Wrapper;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
impl Wrapper {
    #[no_mangle]
    #[cfg(target_arch = "wasm32")]
    pub unsafe fn update_canvas(
        width: wasm_bindgen::JsValue, 
        height: wasm_bindgen::JsValue,
    ) -> Result<(), String> {
        use std::io;

        use winit::dpi::PhysicalSize;
    
        let width = width
            .as_string()
            .ok_or(io::Error::from(io::ErrorKind::InvalidData))
            .map_err(|e| e.to_string())?
            .parse::<u32>()
            .map_err(|e| e.to_string())?;
    
        let height = height
            .as_string()
            .ok_or(io::Error::from(io::ErrorKind::InvalidData))
            .map_err(|e| e.to_string())?
            .parse::<u32>()
            .map_err(|e| e.to_string())?;
    
        unsafe {
            let _ = backend::VIEWPORT.insert(PhysicalSize { width, height });
        }
    
        Ok(())
    }

    #[no_mangle]
    pub async fn run() -> Result<(), String> {
        #[cfg(target_arch = "wasm32")] {
            console_error_panic_hook::set_once();
            wasm_logger::init(wasm_logger::Config::default());
        }
        
        #[cfg(not(target_arch = "wasm32"))] {
            simple_logger::SimpleLogger::new()
                .with_level(log::LevelFilter::Info)
                .init()
                .unwrap();
        }
    
        let config = globe::GlobeConfig::default();
    
        let app= backend::App::<globe::GlobeConfig, globe::Globe>::new(config)
            .await
            .map_err(|e| e.to_string())?;
    
        app.run().map_err(|e| e.to_string())
    }
}