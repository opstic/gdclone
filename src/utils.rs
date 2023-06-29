use base64::DecodeError::InvalidByte;
use std::io::Read;

use base64::Engine;
use bevy::log::info;
use bevy::math::{IVec2, Vec2};
use bevy::prelude::Color;
use bevy::utils::{hashbrown, PassHash};

use crate::level::SECTION_SIZE;

pub type PassHashMap<V> = hashbrown::HashMap<u64, V, PassHash>;

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
pub(crate) fn linear_to_nonlinear(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|val| {
        if val <= 0.0 {
            return val;
        }
        if val <= 0.0031308 {
            // Linear falloff in dark values
            val * 12.92
        } else {
            // Gamma curve in other area
            (1.055 * val.powf(1.0 / 2.4)) - 0.055
        }
    })
}

#[inline(always)]
pub(crate) fn nonlinear_to_linear(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|val| {
        if val <= 0.0 {
            return val;
        }
        if val <= 0.04045 {
            // Linear falloff in dark values
            val / 12.92
        } else {
            // Gamma curve in other area
            ((val + 0.055) / 1.055).powf(2.4)
        }
    })
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

    match (h.floor() as u8) % 6 {
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
    let invalid_byte_end = bytes
        .iter()
        .rposition(|byte| *byte == key.unwrap_or_default())
        .unwrap_or(bytes.len());
    let invalid_byte_start = bytes[..invalid_byte_end]
        .iter()
        .rposition(|byte| {
            !(*byte == key.unwrap_or_default()
                || (*byte ^ key.unwrap_or_default()).is_ascii_whitespace())
        })
        .unwrap_or(bytes.len() - 1)
        + 1;
    let mut xored = Vec::with_capacity(bytes.len());
    xored.extend(match key {
        Some(key) => bytes[..invalid_byte_start]
            .iter()
            .map(|byte| *byte ^ key)
            .collect::<Vec<u8>>(),
        None => bytes[..invalid_byte_start].to_vec(),
    });
    let mut decoded = Vec::new();
    BASE64_URL_SAFE.decode_vec(xored.clone(), &mut decoded)?;
    Ok(decoded)
}

#[inline(always)]
pub(crate) fn decompress(bytes: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    let mut decompressed = Vec::with_capacity(bytes.len() + bytes.len() / 2);
    match flate2::read::GzDecoder::new(bytes).read_to_end(&mut decompressed) {
        Ok(_) => {}
        Err(_) => {
            // Older versions of GD uses just zlib instead
            decompressed.clear();
            flate2::read::ZlibDecoder::new(bytes).read_to_end(&mut decompressed)?;
        }
    }
    Ok(decompressed)
}

#[inline(always)]
pub(crate) fn section_from_pos(pos: Vec2) -> IVec2 {
    IVec2::new((pos.x / SECTION_SIZE) as i32, (pos.y / SECTION_SIZE) as i32)
}

const BASE64_URL_SAFE: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE;
