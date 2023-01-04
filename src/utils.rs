#[inline(always)]
pub(crate) fn u8_to_bool(byte: &[u8]) -> bool {
    matches!(byte, b"1")
}

#[inline(always)]
pub(crate) fn lerp(start: f32, end: f32, x: f32) -> f32 {
    start + (end - start) * x
}
