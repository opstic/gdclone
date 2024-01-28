#import bevy_render::{
    view::View,
}

fn affine2_to_square(affine: mat3x2<f32>) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(affine[0], 0.0, 0.0),
        vec4<f32>(affine[1], 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(affine[2], 0.0, 1.0),
    );
}

@group(0) @binding(0) var<uniform> view: View;

struct VertexInput {
    // NOTE: Instance-rate vertex buffer members prefixed with i_
    @location(0) i_model_row0: vec2<f32>,
    @location(1) i_model_row1: vec2<f32>,
    @location(2) i_model_row2: vec2<f32>,
    @location(3) i_color: vec4<f32>,
    @location(4) i_uv_offset_scale: vec4<f32>,
    @location(5) i_texture_index: u32,
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

    out.clip_position = view.view_proj * affine2_to_square(mat3x2<f32>(
        in.i_model_row0,
        in.i_model_row1,
        in.i_model_row2,
    )) * vec4<f32>(vertex_position, 1.0);

    out.uv = vec2<f32>(vertex_position.xy) * in.i_uv_offset_scale.zw + in.i_uv_offset_scale.xy;

#ifndef ADDITIVE_BLENDING
    out.color = vec4<f32>(in.i_color.rgb * in.i_color.a, in.i_color.a);
#else
    var alpha = pow(in.i_color.a, 2.0);
    out.color = vec4<f32>(in.i_color.rgb * alpha, 0.0);
#endif

    out.texture_index = in.i_texture_index;

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

#ifdef SQUARE_TEXTURE_ALPHA
    color = color * color.a;
#endif
    color = in.color * color;

    return color;
}
