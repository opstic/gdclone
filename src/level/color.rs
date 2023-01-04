use bevy::reflect::Reflect;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub(crate) enum ColorChannel {
    BaseColor(BaseColor),
    CopyColor(CopyColor),
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct BaseColor {
    pub(crate) index: u64,
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) opacity: f32,
    pub(crate) blending: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct CopyColor {
    pub(crate) index: u64,
    pub(crate) copied_index: u64,
    pub(crate) copy_opacity: bool,
    pub(crate) opacity: f32,
    pub(crate) blending: bool,
    pub(crate) hsv: HSV,
}

#[derive(Debug, Deserialize, Clone, Reflect)]
pub(crate) struct HSV {
    pub(crate) h: f32,
    pub(crate) s: f32,
    pub(crate) v: f32,
    pub(crate) checked_s: i64,
    pub(crate) checked_v: i64,
}

impl Default for HSV {
    fn default() -> Self {
        HSV {
            h: 0.,
            s: 1.,
            v: 1.,
            checked_s: 0,
            checked_v: 0,
        }
    }
}

impl Default for BaseColor {
    fn default() -> Self {
        BaseColor {
            index: 0,
            r: 255,
            g: 255,
            b: 255,
            opacity: 1.,
            blending: false,
        }
    }
}

impl Default for CopyColor {
    fn default() -> Self {
        CopyColor {
            index: 0,
            copied_index: 0,
            copy_opacity: false,
            opacity: 1.,
            blending: false,
            hsv: HSV::default(),
        }
    }
}
