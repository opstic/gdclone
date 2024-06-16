#import bevy_render::{
    view::View,
}

fn affine2_to_square(affine: mat3x2<f32>) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(affine.x, 0.0, 0.0),
        vec4<f32>(affine.y, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(affine.z, 0.0, 1.0),
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
    @location(6) i_hsv: vec3<f32>,
    @location(7) i_hsv_flags: u32,
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
#ifndef NO_TEXTURE_ARRAY
    @location(2) texture_index: u32,
#endif
};

// From https://github.com/lolengine/lolengine/blob/3c26/doc/legacy/front_camera_sprite.lolfx#L56
fn rgb2hsv(rgb: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(rgb.bg, K.wz), vec4<f32>(rgb.gb, K.xy), step(rgb.b, rgb.g));
    let q = mix(vec4<f32>(p.xyw, rgb.r), vec4<f32>(rgb.r, p.yzx), step(p.x, rgb.r));

    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

// From https://github.com/lolengine/lolengine/blob/3c26/doc/legacy/front_camera_sprite.lolfx#L67
fn hsv2rgb(hsv: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(hsv.xxx + K.xyz) * 6.0 - K.www);
    return hsv.z * mix(K.xxx, clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), hsv.y);
}

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let vertex_position = vec2<f32>(
        f32(in.index & 1),
        f32((in.index & 2) >> 1),
    );

    out.clip_position = vec4<f32>(vertex_position, 0.0, 1.0)
     * transpose(affine2_to_square(mat3x2<f32>(
        in.i_model_row0,
        in.i_model_row1,
        in.i_model_row2,
    ))) * transpose(view.view_proj);

    out.uv = vertex_position * in.i_uv_offset_scale.zw + in.i_uv_offset_scale.xy;

    var rgb = in.i_color.rgb;

    var hsv = rgb2hsv(in.i_color.rgb);
    var h = hsv.x + in.i_hsv.x;
    let normal_sv = vec2<f32>(
        hsv.y * in.i_hsv.y,
        hsv.z * in.i_hsv.z,
    );
    let absolute_sv = vec2<f32>(
        hsv.y + in.i_hsv.y,
        hsv.z + in.i_hsv.z,
    );
    let absolute_flags = vec2<f32>(
        f32((in.i_hsv_flags >> 1) & 1),
        f32((in.i_hsv_flags >> 2) & 1),
    );

    let sv = mix(normal_sv, absolute_sv, absolute_flags);

    hsv = vec3<f32>(fract(h + 1.0), clamp(sv, vec2<f32>(0.0), vec2<f32>(1.0)));

    let hsv_rgb = hsv2rgb(hsv);

    rgb = mix(hsv_rgb, rgb, vec3<f32>(f32((in.i_hsv_flags >> 0) & 1)));

#ifndef ADDITIVE_BLENDING
    out.color = vec4<f32>(rgb * in.i_color.a, in.i_color.a);
#else
    let alpha = in.i_color.a * in.i_color.a;
    out.color = vec4<f32>(rgb * alpha, 0.0);
#endif

#ifndef NO_TEXTURE_ARRAY
    out.texture_index = in.i_texture_index;
#endif

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
