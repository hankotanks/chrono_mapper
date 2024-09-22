//include shaders::types::camera

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var mercator: texture_2d<f32>;

@group(1) @binding(1)
var mercator_sampler: sampler;

struct VertexInput {
    @location(0) 
    pos: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) 
    pos_clip: vec4<f32>,
    @location(0) 
    pos_world: vec3<f32>,
    @location(1) 
    normal: vec3<f32>,
};

@vertex
fn vertex(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
        out.pos_clip = camera.proj * camera.view * vec4<f32>(model.pos, 1.0);
        out.pos_world = model.pos;
        out.normal = normalize(model.pos);

    return out;
}

const AMBIENCE: f32 = 0.3;

const PI: f32 = 3.1415926535;

const OFFSET: f32 = 0.75;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var light_dir = normalize(camera.eye.xyz - in.pos_world.xyz);

    let diffuse = max(dot(light_dir, in.normal), 0.0);

    let pos = normalize(in.pos_world);
    let tex: vec2<f32> = vec2<f32>(
        ((atan2(pos.x, pos.z) / PI + 1.0) / 2.0 + OFFSET) % 1.0, asin(pos.y) / PI + 0.5,
    );

    let color = textureSample(mercator, mercator_sampler, tex).xyz * //
        (diffuse + AMBIENCE);
    
    return vec4<f32>(color, 1.0);
}
