#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

#import bevy_render::view

@group(0) @binding(0)
var<uniform> view: View;

struct VertexInput {
    @location(0) anchor: vec2<f32>,
    @location(1) half_extents: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) transform_x: vec3<f32>,
    @location(4) transform_y: vec3<f32>,
    @location(5) transform_z: vec3<f32>,
    @location(6) transform_w: vec3<f32>,
    @location(7) color: vec4<f32>,
    @location(8) texture_index: u32,
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) texture_index: u32,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var xy = vec2<f32>(f32(in.index & 1u), f32((in.index & 2u) >> 1u));
    var position = ((xy * 2.0 - 1.0) - in.anchor) * in.half_extents;
    let transform_matrix = mat4x4<f32>(
        vec4<f32>(in.transform_x, 0.0),
        vec4<f32>(in.transform_y, 0.0),
        vec4<f32>(in.transform_z, 0.0),
        vec4<f32>(in.transform_w, 1.0),
    );
    var out: VertexOutput;
    out.uv = in.uv.xy + in.uv.zw * (1.0 - xy);
    out.position = view.view_proj * transform_matrix * vec4<f32>(position, 0.0, 1.0);
    out.color = in.color;
    out.texture_index = in.texture_index;
    return out;
}

@group(1) @binding(0)
var sprite_textures: binding_array<texture_2d<f32>, 16>;
@group(1) @binding(1)
var sprite_samplers: binding_array<sampler, 16>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(sprite_textures[in.texture_index], sprite_samplers[in.texture_index], in.uv);
    color = in.color * color;

#ifdef TONEMAP_IN_SHADER
    color = tone_mapping(color);
#endif

    return color;
}
