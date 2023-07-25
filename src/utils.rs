use bevy::math::{IVec2, Vec2};
use bevy::prelude::Color;
use bevy::tasks::ComputeTaskPool;
use bevy::utils::{hashbrown, PassHash};
use libdeflater::Decompressor;

use crate::level::SECTION_SIZE;

pub type PassHashMap<V> = hashbrown::HashMap<u64, V, PassHash>;
pub type PassHashSet = hashbrown::HashSet<u64, PassHash>;

#[inline(always)]
pub(crate) fn u8_to_bool(byte: &[u8]) -> bool {
    matches!(byte, b"1")
}

#[inline(always)]
pub(crate) fn lerp(start: &f32, end: &f32, x: &f32) -> f32 {
    start + (end - start) * x
}

// https://github.com/bevyengine/bevy/issues/6315#issuecomment-1332720260
#[inline(always)]
pub(crate) fn linear_to_nonlinear(val: f32) -> f32 {
    if val <= 0.0031308 {
        // Linear falloff in dark values
        val * 12.92
    } else {
        // Gamma curve in other area
        (1.055 * val.powf(1.0 / 2.4)) - 0.055
    }
}

#[inline(always)]
pub(crate) fn nonlinear_to_linear(val: f32) -> f32 {
    if val <= 0.04045 {
        // Linear falloff in dark values
        val / 12.92
    } else {
        // Gamma curve in other area
        ((val + 0.055) / 1.055).powf(2.4)
    }
}

#[inline(always)]
pub(crate) fn fast_scale(val: u8, x: u8) -> u8 {
    let r1 = val as u16 * x as u16 + 128;
    (((r1 >> 8) + r1) >> 8) as u8
}

#[inline(always)]
pub fn rgb_to_hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);

    let delta = max - min;

    let mut h = if r == max {
        // Between yellow & magenta
        (g - b) / delta
    } else if g == max {
        // Between cyan & yellow
        2. + (b - r) / delta
    } else {
        // Between magenta & cyan
        4. + (r - g) / delta
    };

    // To degrees
    h *= 60.;

    h = h.rem_euclid(360.);

    (h, if max == 0. { 0. } else { delta / max }, max)
}

#[inline(always)]
pub fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> [f32; 3] {
    if h.is_nan() {
        return [v, v, v];
    }

    let h = h.rem_euclid(360.);
    let s = s.clamp(0., 1.);
    let v = v.clamp(0., 1.);

    let h = h / 60.;
    let p = v * (1. - s);
    let q = v * (1. - (s * h.fract()));
    let t = v * (1. - (s * (1. - h.fract())));

    match (h as u8) % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        5 => [v, p, q],
        _ => unreachable!(),
    }
}

#[inline(always)]
pub(crate) fn lerp_color(start: &Color, end: &Color, x: &f32) -> Color {
    let r = lerp(&start.r(), &end.r(), x);
    let g = lerp(&start.g(), &end.g(), x);
    let b = lerp(&start.b(), &end.b(), x);
    let a = lerp(&start.a(), &end.a(), x);
    Color::rgba(r, g, b, a)
}

#[inline(always)]
pub(crate) fn decrypt(bytes: &[u8], key: Option<u8>) -> Result<Vec<u8>, anyhow::Error> {
    let invalid_bytes_end = bytes
        .iter()
        .rposition(|byte| *byte == key.unwrap_or_default())
        .unwrap_or(bytes.len());
    let invalid_bytes_start = bytes[..invalid_bytes_end]
        .iter()
        .rposition(|byte| {
            !(*byte == key.unwrap_or_default()
                || (*byte ^ key.unwrap_or_default()).is_ascii_whitespace())
        })
        .unwrap_or(bytes.len() - 1)
        + 1;

    let Some(key) = key else {
        return Ok(base64_simd::URL_SAFE.decode_to_vec(&bytes[..invalid_bytes_start])?);
    };

    let task_pool = ComputeTaskPool::get();

    let actual_encoded_len = bytes[..invalid_bytes_start]
        .iter()
        .rposition(|byte| *byte != b'=' ^ key)
        .unwrap_or_default();

    let mut decode_output = vec![0; actual_encoded_len / 4 * 3 + actual_encoded_len % 4];

    let mut thread_chunk_size = decode_output.len() / task_pool.thread_num();
    thread_chunk_size -= thread_chunk_size % 192;

    task_pool.scope(|scope| {
        for (current_chunk, chunk) in decode_output.chunks_mut(thread_chunk_size).enumerate() {
            scope.spawn(async move {
                let mut encoded_start = current_chunk * thread_chunk_size;
                encoded_start = encoded_start / 3 * 4;
                let encoded_end =
                    (encoded_start + thread_chunk_size / 3 * 4).min(invalid_bytes_start);
                let encoded = &bytes[encoded_start..encoded_end];

                let mut temp = [0; 256];

                for (encoded_chunk, decoded_chunk) in encoded.chunks(256).zip(chunk.chunks_mut(192))
                {
                    let len = temp.len().min(encoded_chunk.len());
                    let temp = &mut temp[..len];
                    temp.copy_from_slice(encoded_chunk);
                    for byte in &mut *temp {
                        *byte ^= key;
                    }
                    base64_simd::URL_SAFE
                        .decode(temp, base64_simd::Out::from_slice(decoded_chunk))
                        .unwrap();
                }
            })
        }
    });

    Ok(decode_output)
}

#[inline(always)]
pub(crate) fn decompress(bytes: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let decompressed_size_data = &bytes[bytes.len() - 4..];
    let mut decompressed_size: u32 = decompressed_size_data[0] as u32;
    decompressed_size |= (decompressed_size_data[1] as u32) << 8;
    decompressed_size |= (decompressed_size_data[2] as u32) << 16;
    decompressed_size |= (decompressed_size_data[3] as u32) << 24;

    let mut decompressed = Vec::new();
    decompressed.resize(decompressed_size as usize, 0);

    let mut decompressor = Decompressor::new();

    if decompressor
        .gzip_decompress(bytes, &mut decompressed)
        .is_err()
    {
        decompressed.clear();
        decompressed.resize(decompressed_size as usize, 0);
        decompressor.zlib_decompress(bytes, &mut decompressed)?;
    }

    Ok(decompressed)
}

#[inline(always)]
pub(crate) fn section_from_pos(pos: Vec2) -> IVec2 {
    IVec2::new((pos.x / SECTION_SIZE) as i32, (pos.y / SECTION_SIZE) as i32)
}
