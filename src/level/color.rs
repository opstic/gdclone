use crate::level::de;
use bevy::prelude::{Color, Component, Entity, Resource};
use bevy::reflect::Reflect;
use bevy_enum_filter::EnumFilter;
use dashmap::{DashMap, DashSet};
use serde::Deserialize;

use crate::utils::{u8_to_bool, U64Hash};

#[derive(Resource)]
struct GlobalColorChannels(DashMap<u64, (Entity, DashSet<Entity, U64Hash>), U64Hash>);

#[derive(Component)]
struct GlobalColorChannel {
    id: u64,
    kind: ColorChannelKind,
}

#[derive(Default, Component)]
pub(crate) struct ObjectColor {
    pub(crate) channel_id: u64,
    pub(crate) hsv: Option<HsvMod>,
    pub(crate) opacity: f32,
    pub(crate) color: Color,
}

#[derive(Default, Copy, Clone, Component, EnumFilter)]
pub(crate) enum ObjectColorKind {
    Base,
    Detail,
    Black,
    #[default]
    None,
}

enum ColorChannelKind {
    Base {
        color: Color,
        blending: bool,
    },
    Copy {
        copied_index: u64,
        copy_opacity: bool,
        opacity: f32,
        blending: bool,
        hsv: HsvMod,
    },
}

#[derive(Component, Debug, Deserialize, Copy, Clone, Reflect)]
pub(crate) struct HsvMod {
    pub(crate) h: f32,
    pub(crate) s: f32,
    pub(crate) v: f32,
    pub(crate) s_absolute: bool,
    pub(crate) v_absolute: bool,
}

impl HsvMod {
    pub(crate) fn parse(hsv_string: &[u8]) -> Result<HsvMod, anyhow::Error> {
        let hsv_data: [&[u8]; 5] = de::from_slice(hsv_string, b'a')?;
        Ok(HsvMod {
            h: std::str::from_utf8(hsv_data[0])?.parse()?,
            s: std::str::from_utf8(hsv_data[1])?.parse()?,
            v: std::str::from_utf8(hsv_data[2])?.parse()?,
            s_absolute: u8_to_bool(hsv_data[3]),
            v_absolute: u8_to_bool(hsv_data[4]),
        })
    }
}

impl Default for HsvMod {
    fn default() -> Self {
        HsvMod {
            h: 0.,
            s: 1.,
            v: 1.,
            s_absolute: false,
            v_absolute: false,
        }
    }
}
