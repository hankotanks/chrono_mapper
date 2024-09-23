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
    surface_format: wgpu::TextureFormat::Rgba8Unorm,
    slices: 100,
    stacks: 100,
    globe_radius: 10000.,
    globe_shader_asset_path: "shaders/render_basemap.wgsl",
    basemap: "blue_marble_2048.tif", // https://visibleearth.nasa.gov/images/57752/blue-marble-land-surface-shallow-water-and-shaded-topography
    basemap_padding: winit::dpi::PhysicalSize { width: 0, height: 0 },
    features: &[
        // https://github.com/aourednik/historical-basemaps/tree/master
        "features/world_100.geojson",
        "features/world_200.geojson",
        "features/world_300.geojson",
        "features/world_400.geojson",
        "features/world_500.geojson",
        "features/world_600.geojson",
        "features/world_700.geojson",
        "features/world_800.geojson",
        "features/world_900.geojson",
        "features/world_1000.geojson",
        "features/world_1100.geojson",
        "features/world_1200.geojson",
        "features/world_1279.geojson",
        "features/world_1300.geojson",
        "features/world_1400.geojson",
        "features/world_1492.geojson",
        "features/world_1500.geojson",
        "features/world_1530.geojson",
        "features/world_1600.geojson",
        "features/world_1650.geojson",
        "features/world_1700.geojson",
        "features/world_1715.geojson",
        "features/world_1783.geojson",
        "features/world_1800.geojson",
        "features/world_1815.geojson",
        "features/world_1880.geojson",
        "features/world_1900.geojson",
        "features/world_1920.geojson",
        "features/world_1930.geojson",
        "features/world_1938.geojson",
        "features/world_1945.geojson",
        "features/world_1960.geojson",
        "features/world_1994.geojson",
        "features/world_2000.geojson",
        "features/world_2010.geojson",
    ],
    features_shader_asset_path: "shaders/render_features.wgsl",
    debug_font_asset_path: "fonts/biolinium.ttf",
};