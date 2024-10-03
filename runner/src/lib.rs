#[backend::init(app::App, app::Config => CONFIG)]
pub struct Wrapper;

const fn ext(path: &'static str) -> backend::AssetRef<'static> {
    backend::AssetRef { path, locator: backend::AssetLocator::Local }
}

const CONFIG: app::Config = app::Config { 
    surface_format: backend::wgpu::TextureFormat::Rgba8Unorm,
    font_asset_path: "fonts/biolinium.ttf",
    font_family: "Linux Biolinium G",
    slices: 100,
    stacks: 100,
    globe_radius: 10000.,
    globe_shader_asset_path: "shaders/render_basemap.wgsl",
    // https://visibleearth.nasa.gov/images/57752/blue-marble-land-surface-shallow-water-and-shaded-topography
    basemap: "blue_marble_2048.tif", 
    basemap_padding: backend::Size { width: 0, height: 0 },
    features: &[
        // https://github.com/aourednik/historical-basemaps/tree/master
        ext("features/world_100.geojson"),
        ext("features/world_200.geojson"),
        ext("features/world_300.geojson"),
        ext("features/world_400.geojson"),
        ext("features/world_500.geojson"),
        ext("features/world_600.geojson"),
        ext("features/world_700.geojson"),
        ext("features/world_800.geojson"),
        ext("features/world_900.geojson"),
        ext("features/world_1000.geojson"),
        ext("features/world_1100.geojson"),
        ext("features/world_1200.geojson"),
        ext("features/world_1279.geojson"),
        ext("features/world_1300.geojson"),
        ext("features/world_1400.geojson"),
        // TODO: A better approach to multi-layered features
        // "features/world_1492.geojson",
        ext("features/world_1500.geojson"),
        ext("features/world_1530.geojson"),
        ext("features/world_1600.geojson"),
        ext("features/world_1650.geojson"),
        ext("features/world_1700.geojson"),
        ext("features/world_1715.geojson"),
        ext("features/world_1783.geojson"),
        ext("features/world_1800.geojson"),
        ext("features/world_1815.geojson"),
        ext("features/world_1880.geojson"),
        ext("features/world_1900.geojson"),
        ext("features/world_1920.geojson"),
        ext("features/world_1930.geojson"),
        ext("features/world_1938.geojson"),
        ext("features/world_1945.geojson"),
        ext("features/world_1960.geojson"),
        ext("features/world_1994.geojson"),
        ext("features/world_2000.geojson"),
        ext("features/world_2010.geojson"),
    ],
    features_shader_asset_path: "shaders/render_features.wgsl",
    feature_label_ray_density: 15,
};