use base64::Engine;
use bevy::prelude::Color;
use std::io::Read;

#[inline(always)]
pub(crate) fn u8_to_bool(byte: &[u8]) -> bool {
    matches!(byte, b"1")
}

#[inline(always)]
pub(crate) fn lerp(start: &f64, end: &f64, x: &f64) -> f64 {
    start + (end - start) * x
}

#[inline(always)]
pub fn rgb_to_hsv(rgb: [f32; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb;
    let (max, min, diff, add) = {
        let (max, min, diff, add) = if r > g {
            (r, g, g - b, 0.0)
        } else {
            (g, r, b - r, 2.0)
        };
        if b > max {
            (b, min, r - g, 4.0)
        } else {
            (max, b.min(min), diff, add)
        }
    };

    let v = max;
    let h = if max == min {
        0.0
    } else {
        let mut h = 60.0 * (add + diff / (max - min));
        if h < 0.0 {
            h += 360.0;
        }
        h
    };
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };

    (h, s, v)
}

/// Convert hsv to rgb. Expects h [0, 360], s [0, 1], v [0, 1]
#[inline(always)]
pub fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> [f32; 3] {
    let c = s * v;
    let h = h / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if (0.0..=1.0).contains(&h) {
        (c, x, 0.0)
    } else if h <= 2.0 {
        (x, c, 0.0)
    } else if h <= 3.0 {
        (0.0, c, x)
    } else if h <= 4.0 {
        (0.0, x, c)
    } else if h <= 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r + m, g + m, b + m]
}

pub(crate) fn lerp_color(start: &Color, end: &Color, x: &f64) -> Color {
    let r = lerp(&(start.r() as f64), &(end.r() as f64), x) as f32;
    let g = lerp(&(start.g() as f64), &(end.g() as f64), x) as f32;
    let b = lerp(&(start.b() as f64), &(end.b() as f64), x) as f32;
    let a = lerp(&(start.a() as f64), &(end.a() as f64), x) as f32;
    Color::rgba(r, g, b, a)
}

pub(crate) fn decrypt(bytes: &[u8], key: Option<u8>) -> Result<Vec<u8>, anyhow::Error> {
    let mut xored = Vec::with_capacity(bytes.len());
    let nul_byte_start = bytes
        .iter()
        .rposition(|byte| *byte != key.unwrap_or_default())
        .unwrap_or(bytes.len() - 1);
    xored.extend(match key {
        Some(key) => bytes[..nul_byte_start + 1]
            .iter()
            .map(|byte| *byte ^ key)
            .collect::<Vec<u8>>(),
        None => bytes[..nul_byte_start + 1].to_vec(),
    });
    let mut decoded = Vec::new();
    BASE64_URL_SAFE.decode_vec(xored, &mut decoded)?;
    Ok(decoded)
}

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

const BASE64_URL_SAFE: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE;
