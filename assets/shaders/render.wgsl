struct CameraUniform {
    eye: vec4<f32>,
    view: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) pos_clip: vec4<f32>,
    @location(0) pos_world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

@vertex
fn vertex(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
        out.pos_clip = camera.view * vec4<f32>(model.pos, 1.0);
        out.pos_world = model.pos;
        out.normal = normalize(model.pos);
        out.color = model.color;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var light_dir = normalize(camera.eye.xyz - in.pos_world.xyz);

    let intensity_diffuse = max(dot(light_dir, in.normal), 0.0);

    let color = in.color * (intensity_diffuse + 0.2);
    
    return vec4<f32>(color, 1.0);
}
