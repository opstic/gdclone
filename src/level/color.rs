use bevy::prelude::{Color, Component, Entity, Resource};
use bevy::reflect::Reflect;
use bevy_enum_filter::EnumFilter;
use dashmap::{DashMap, DashSet};
use serde::Deserialize;

use crate::utils::U64Hash;

#[derive(Resource)]
struct GlobalColorChannels(DashMap<u64, (Entity, DashSet<Entity, U64Hash>), U64Hash>);

#[derive(Component)]
struct GlobalColorChannel {
    id: u64,
    kind: ColorChannelKind,
}

#[derive(Component)]
struct ColorChannel {
    id: u64,
    kind: ColorKind,
}

#[derive(Default, Component, EnumFilter)]
pub(crate) enum ColorKind {
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
