//include shaders/types/camera.wgsl

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) 
    pos: vec3<f32>,
    @location(1)
    color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) 
    pos_clip: vec4<f32>,
    @location(0)
    color: vec3<f32>,
};

@vertex
fn vertex(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
        out.pos_clip = camera.proj * camera.view * vec4<f32>(model.pos, 1.0);
        out.color = model.color;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 0.6);
}