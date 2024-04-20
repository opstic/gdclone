use std::borrow::Borrow;
use std::hash::BuildHasher;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Sub};

use arrayvec::ArrayVec;
use bevy::ecs::entity::EntityHasher;
use bevy::log::{info, warn};
use bevy::math::{Vec3A, Vec4, Vec4Swizzles};
use bevy::tasks::AsyncComputeTaskPool;
use libdeflater::Decompressor;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

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

/// A *very* limited map based on ['ArrayVec'] that only works with inserts of unique elements
/// Anything else would break it
#[derive(Clone, Debug)]
pub(crate) struct ArrayMap<K, V, const N: usize> {
    storage: ArrayVec<(K, V), N>,
}

impl<K, V, const N: usize> ArrayMap<K, V, N>
where
    K: Eq,
{
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            storage: ArrayVec::new_const(),
        }
    }

    #[inline]
    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.storage.try_push((key, value)).unwrap()
    }

    #[inline]
    pub(crate) fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Eq,
    {
        match self
            .storage
            .iter()
            .find(|(stored_key, _)| stored_key.borrow() == key)
        {
            Some((_, v)) => Some(v),
            None => None,
        }
    }
}

impl<'de, K, V, const N: usize> Deserialize<'de> for ArrayMap<K, V, N>
where
    K: Eq + Deserialize<'de>,
    V: Deserialize<'de>,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<ArrayMap<K, V, N>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(HashMapVisitor {
            marker: PhantomData,
        })
    }
}

struct HashMapVisitor<K, V, const N: usize>
where
    K: Eq,
{
    marker: PhantomData<ArrayMap<K, V, N>>,
}

impl<'de, K, V, const N: usize> Visitor<'de> for HashMapVisitor<K, V, N>
where
    K: Eq + Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = ArrayMap<K, V, N>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an Object/Map structure")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut m = ArrayMap::new();
        while let Some(k) = map.next_key()? {
            let v = map.next_value()?;
            m.insert(k, v);
        }
        Ok(m)
    }
}

pub(crate) type StartObjectStorage<'decompressed> =
    ArrayMap<&'decompressed str, &'decompressed str, 48>;
pub(crate) type ObjectStorage<'decompressed> = ArrayMap<&'decompressed str, &'decompressed str, 45>;

#[inline]
pub(crate) fn str_to_bool(string: &str) -> bool {
    string == "1"
}

#[inline]
pub(crate) fn lerp<T: Copy + Mul<f32, Output = T> + Add<T, Output = T> + Sub<T, Output = T>>(
    start: T,
    end: T,
    x: f32,
) -> T {
    start + (end - start) * x
}

#[inline]
pub(crate) fn lerp_start<
    T: Copy + Mul<f32, Output = T> + Div<f32, Output = T> + Sub<T, Output = T>,
>(
    current: T,
    end: T,
    x: f32,
) -> T {
    (current - end * x) / (1. - x + 1e-45)
}

#[inline(always)]
pub(crate) const fn fast_scale(val: u8, x: u8) -> u8 {
    let r1 = val as u16 * x as u16 + 128;
    (((r1 >> 8) + r1) >> 8) as u8
}

// From https://github.com/lolengine/lol/blob/b5f0/include/lol/private/image/color.h#L146
#[inline]
pub(crate) fn rgb_to_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let mut k = 0.;

    let [mut r, mut g, mut b] = rgb;

    if g < b {
        std::mem::swap(&mut g, &mut b);
        k = -1.;
    }

    let mut min_gb = b;

    if r < g {
        std::mem::swap(&mut r, &mut g);
        k = -2. / 6. - k;
        min_gb = g.min(b);
    }

    let chroma = r - min_gb;

    [
        (k + (g - b) / (chroma * 6. + 1e-45)).abs(),
        chroma / (r + 1e-45),
        r,
    ]
}

#[inline]
pub(crate) fn hsv_to_rgb([h, s, v]: [f32; 3]) -> [f32; 3] {
    let h = (h.fract() + if h < 0. { 1. } else { 0. }) * 6.;
    let h_fract = h.fract();
    let s = s.clamp(0., 1.);
    let v = v.clamp(0., 1.);

    let mut pqt = Vec3A::new(1. - s, 1. - (s * h_fract), 1. - (s * (1. - h_fract)));

    pqt *= v;

    match (h as u8) % 6 {
        0 => [v, pqt.z, pqt.x],
        1 => [pqt.y, v, pqt.x],
        2 => [pqt.x, v, pqt.z],
        3 => [pqt.x, pqt.y, v],
        4 => [pqt.z, pqt.x, v],
        5 => [v, pqt.x, pqt.y],
        _ => unreachable!(),
    }
}

#[inline]
pub(crate) fn intersect_aabb(a: Vec4, b: Vec4) -> bool {
    a.cmple(-b.zwxy()).all()
}

const SECTION_SIZE_POWER: u32 = 7;
const SECTION_SIZE: f32 = 2_u32.pow(SECTION_SIZE_POWER) as f32;

#[inline(always)]
pub(crate) fn section_index_from_x(x: f32) -> u32 {
    (x / SECTION_SIZE) as u32
}

#[inline]
pub(crate) fn decrypt<const KEY: u8>(bytes: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    const BUFFER_SIZE: usize = 1024;
    const RPOSITION_LIMIT: usize = 4;

    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let invalid_bytes_end = bytes[bytes.len().saturating_sub(RPOSITION_LIMIT)..]
        .iter()
        .rposition(|byte| *byte == KEY)
        .map(|found_index| (found_index + bytes.len()).saturating_sub(RPOSITION_LIMIT))
        .unwrap_or(bytes.len());
    let invalid_bytes_start = bytes
        [invalid_bytes_end.saturating_sub(RPOSITION_LIMIT)..invalid_bytes_end]
        .iter()
        .rposition(|byte| !(*byte == KEY || (*byte ^ KEY).is_ascii_whitespace()))
        .map(|found_index| (found_index + invalid_bytes_end).saturating_sub(RPOSITION_LIMIT))
        .unwrap_or(bytes.len().saturating_sub(1))
        + 1;

    let base64_padding = b'=' ^ KEY;

    let actual_encoded_len = bytes
        [invalid_bytes_start.saturating_sub(RPOSITION_LIMIT)..invalid_bytes_start]
        .iter()
        .rposition(|byte| *byte != base64_padding)
        .map(|found_index| (found_index + invalid_bytes_start).saturating_sub(RPOSITION_LIMIT))
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
                    let temp_subslice = &mut temp[..encoded_chunk.len()];
                    temp_subslice.copy_from_slice(encoded_chunk);
                    for byte in &mut temp {
                        *byte ^= KEY;
                    }
                    base64_simd::URL_SAFE
                        .decode(
                            &temp[..encoded_chunk.len()],
                            base64_simd::Out::from_slice(decoded_chunk),
                        )
                        .unwrap();
                }
            })
        }
    });

    Ok(decode_output)
}

#[inline]
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
