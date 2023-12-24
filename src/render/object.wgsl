#import bevy_render::{
    maths::affine_to_square,
    view::View,
}

@group(0) @binding(0) var<uniform> view: View;

struct VertexInput {
    // NOTE: Instance-rate vertex buffer members prefixed with i_
    // NOTE: i_model_transpose_colN are the 3 columns of a 3x4 matrix that is the transpose of the
    // affine 4x3 model matrix.
    @location(0) i_model_transpose_col0: vec4<f32>,
    @location(1) i_model_transpose_col1: vec4<f32>,
    @location(2) i_model_transpose_col2: vec4<f32>,
    @location(3) i_color: vec4<f32>,
    @location(4) i_uv_offset_scale: vec4<f32>,
    @location(5) i_texture_index: u32,
    @location(6) i_padding: vec3<u32>,
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) texture_index: u32,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let vertex_position = vec3<f32>(
        f32(in.index & 0x1u),
        f32((in.index & 0x2u) >> 1u),
        0.0
    );

    out.clip_position = view.view_proj * affine_to_square(mat3x4<f32>(
        in.i_model_transpose_col0,
        in.i_model_transpose_col1,
        in.i_model_transpose_col2,
    )) * vec4<f32>(vertex_position, 1.0);
    out.uv = vec2<f32>(vertex_position.xy) * in.i_uv_offset_scale.zw + in.i_uv_offset_scale.xy;
    out.color = in.i_color;

    return out;
}

#ifndef NO_TEXTURE_ARRAY
@group(1) @binding(0) var sprite_textures: binding_array<texture_2d<f32>, 16>;
@group(1) @binding(1) var sprite_samplers: binding_array<sampler, 16>;
#else
@group(1) @binding(0) var sprite_texture: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;
#endif

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifndef NO_TEXTURE_ARRAY
    var color = textureSample(sprite_textures[in.texture_index], sprite_samplers[in.texture_index], in.uv);
#else
    var color = textureSample(sprite_texture, sprite_sampler, in.uv);
#endif
    color = in.color * color;

    return color;
}