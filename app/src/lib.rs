mod globe;

type App<'a> = backend::App::<'a, globe::GlobeConfig<'static>, globe::Globe>;

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
        (App::new(CONFIG).await)?.run()
    }
}

const CONFIG: globe::GlobeConfig = globe::GlobeConfig { 
    format: wgpu::TextureFormat::Rgba8Unorm,
    slices: 100,
    stacks: 100,
    globe_radius: 10000.,
    globe_shader_asset_path: "shaders::render_basemap",
    basemap: "blue_marble_2048.tif", // https://visibleearth.nasa.gov/images/57752/blue-marble-land-surface-shallow-water-and-shaded-topography
    basemap_padding: winit::dpi::PhysicalSize { width: 0, height: 0 },
    features: &[
        // https://github.com/aourednik/historical-basemaps/tree/master
        "features::world_100",
        "features::world_200",
        "features::world_300",
        "features::world_400",
        "features::world_500",
        "features::world_600",
        "features::world_700",
        "features::world_800",
        "features::world_900",
        "features::world_1000",
        "features::world_1100",
        "features::world_1200",
        "features::world_1279",
        "features::world_1300",
        "features::world_1400",
        "features::world_1492",
        "features::world_1500",
        "features::world_1530",
        "features::world_1600",
        "features::world_1650",
        "features::world_1700",
        "features::world_1715",
        "features::world_1783",
        "features::world_1800",
        "features::world_1815",
        "features::world_1880",
        "features::world_1900",
        "features::world_1920",
        "features::world_1930",
        "features::world_1938",
        "features::world_1945",
        "features::world_1960",
        "features::world_1994",
        "features::world_2000",
        "features::world_2010",
    ],
    features_shader_asset_path: "shaders::render_features",
};