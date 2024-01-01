use std::hash::{BuildHasher, Hash};

use bevy::log::{info, warn};
use bevy::tasks::AsyncComputeTaskPool;
use bevy::utils::EntityHasher;
use dashmap::DashMap;
use libdeflater::Decompressor;

/// A copy of [`bevy::utils::EntityHash`] with [`Clone`] derived
///
/// Since it's main goal is hashing [`u64`]s effectively it should also
/// be capable of being used on any other [`u64`]-based types
#[derive(Clone, Default)]
pub struct U64Hash;

impl BuildHasher for U64Hash {
    type Hasher = EntityHasher;

    fn build_hasher(&self) -> Self::Hasher {
        EntityHasher::default()
    }
}

/// Extremely unsafe way of reading an entry in the [`DashMap`] without getting a lock
///
/// Improves performance slightly
///
/// Don't use if you can't guarantee there aren't any other threads writing to the [`DashMap`]
pub(crate) unsafe fn dashmap_get_dirty<'map, K: Eq + Hash, V, S: BuildHasher + Clone>(
    key: &K,
    map: &'map DashMap<K, V, S>,
) -> Option<&'map V> {
    let hash = map.hash_usize(key);
    let index = map.determine_shard(hash);

    let shard = unsafe { &*map.shards().get_unchecked(index).data_ptr() };

    if let Some((_, vptr)) = shard.get_key_value(key) {
        unsafe {
            let vptr: *const V = vptr.get();
            Some(&*vptr)
        }
    } else {
        None
    }
}

#[inline(always)]
pub(crate) const fn u8_to_bool(byte: &[u8]) -> bool {
    matches!(byte, b"1")
}

#[inline(always)]
pub(crate) fn lerp(start: f32, end: f32, x: f32) -> f32 {
    start + (end - start) * x
}

#[inline(always)]
pub(crate) fn lerp_start(current: f32, end: f32, x: f32) -> f32 {
    (current - end * x) / (1. - x)
}

#[inline(always)]
pub(crate) const fn fast_scale(val: u8, x: u8) -> u8 {
    let r1 = val as u16 * x as u16 + 128;
    (((r1 >> 8) + r1) >> 8) as u8
}

#[inline(always)]
pub(crate) fn rgb_to_hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
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
pub(crate) fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> [f32; 3] {
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
pub(crate) fn decrypt<const KEY: u8>(bytes: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    const BUFFER_SIZE: usize = 512;
    const RPOSITION_LIMIT: usize = 8;

    let invalid_bytes_end = bytes[bytes.len() - RPOSITION_LIMIT..]
        .iter()
        .rposition(|byte| *byte == KEY)
        .map(|found_index| found_index + bytes.len() - RPOSITION_LIMIT)
        .unwrap_or(bytes.len());
    let invalid_bytes_start = bytes[invalid_bytes_end - RPOSITION_LIMIT..invalid_bytes_end]
        .iter()
        .rposition(|byte| !(*byte == KEY || (*byte ^ KEY).is_ascii_whitespace()))
        .map(|found_index| found_index + invalid_bytes_end - RPOSITION_LIMIT)
        .unwrap_or(bytes.len() - 1)
        + 1;

    let base64_padding = b'=' ^ KEY;

    let actual_encoded_len = bytes[invalid_bytes_start - RPOSITION_LIMIT..invalid_bytes_start]
        .iter()
        .rposition(|byte| *byte != base64_padding)
        .map(|found_index| found_index + invalid_bytes_start - RPOSITION_LIMIT)
        .ok_or(anyhow::Error::msg(
            "Data contains nothing but Base64 padding????",
        ))?;

    let mut decode_output = vec![0; actual_encoded_len / 4 * 3 + actual_encoded_len % 4];

    let task_pool = AsyncComputeTaskPool::get();

    let mut thread_chunk_size = bytes.len() / task_pool.thread_num();
    if thread_chunk_size % BUFFER_SIZE != 0 {
        thread_chunk_size += BUFFER_SIZE - thread_chunk_size % BUFFER_SIZE;
    }

    task_pool.scope(|scope| {
        for (encoded, decoded) in bytes
            .chunks(thread_chunk_size)
            .zip(decode_output.chunks_mut(thread_chunk_size / 4 * 3))
        {
            scope.spawn(async move {
                if KEY == 0 {
                    base64_simd::URL_SAFE
                        .decode(encoded, base64_simd::Out::from_slice(decoded))
                        .unwrap();
                    return;
                };

                let mut temp = [0; BUFFER_SIZE];
                for (encoded_chunk, decoded_chunk) in encoded
                    .chunks(BUFFER_SIZE)
                    .zip(decoded.chunks_mut(BUFFER_SIZE / 4 * 3))
                {
                    let len = temp.len().min(encoded_chunk.len());
                    let temp = &mut temp[..len];
                    temp.copy_from_slice(encoded_chunk);
                    for byte in &mut *temp {
                        *byte ^= 11;
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

    let mut decompressed = vec![0; decompressed_size as usize];

    let mut decompressor = Decompressor::new();

    let gzip_decompress_result = decompressor.gzip_decompress(bytes, &mut decompressed);

    if let Err(decompression_error) = gzip_decompress_result {
        warn!("Gzip decompression failed: {:?}", decompression_error);
        info!("Attempting zlib decompression...");
        decompressed.clear();
        decompressed.resize(decompressed_size as usize, 0);
        decompressor.zlib_decompress(bytes, &mut decompressed)?;
    }

    Ok(decompressed)
}
