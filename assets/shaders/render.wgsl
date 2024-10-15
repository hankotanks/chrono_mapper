//include shaders/types/camera.wgsl

struct VertexInput {
    @location(0) 
    pos: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) 
    pos_clip: vec4<f32>,
};

@vertex
fn vertex(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
        out.pos_clip = vec4<f32>(model.pos, 1.0);

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}