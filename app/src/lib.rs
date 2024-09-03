mod globe;

type App<'a> = backend::App::<'a, globe::GlobeConfig, globe::Globe>;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub struct Wrapper;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
impl Wrapper {
    #[no_mangle]
    #[cfg(target_arch = "wasm32")]
    pub fn update_canvas(
        w: wasm_bindgen::JsValue, 
        h: wasm_bindgen::JsValue,
    ) -> Result<(), String> { App::update_canvas(w, h) }

    #[no_mangle]
    pub async fn run() -> Result<(), String> {
        let config = globe::GlobeConfig::default();

        (App::new(config).await)?.run()
    }
}