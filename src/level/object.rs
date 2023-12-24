use crate::level::color::HsvMod;
use crate::level::section::{GlobalSections, SectionIndex};
use crate::utils::u8_to_bool;
use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{Component, Entity, GlobalTransform, Transform, World};
use bevy::utils::HashMap;

use crate::level::color::{ObjectColor, ObjectColorKind};

struct ObjectDefaultData {
    texture: &'static str,
    default_z_layer: i32,
    default_z_order: i32,
    default_base_color_channel: u64,
    default_detail_color_channel: u64,
    color_kind: ObjectColorKind,
    swap_base_detail: bool,
    opacity: f32,
    children: &'static [ObjectChild],
}

impl ObjectDefaultData {
    const DEFAULT: ObjectDefaultData = ObjectDefaultData {
        texture: "emptyFrame.png",
        default_z_layer: 0,
        default_z_order: 0,
        default_base_color_channel: u64::MAX,
        default_detail_color_channel: u64::MAX,
        color_kind: ObjectColorKind::None,
        swap_base_detail: false,
        opacity: 1.,
        children: &[],
    };
}

impl Default for ObjectDefaultData {
    fn default() -> Self {
        ObjectDefaultData {
            texture: "emptyFrame.png",
            default_z_layer: 0,
            default_z_order: 0,
            default_base_color_channel: u64::MAX,
            default_detail_color_channel: u64::MAX,
            color_kind: ObjectColorKind::None,
            swap_base_detail: false,
            opacity: 1.,
            children: &[],
        }
    }
}

struct ObjectChild {
    texture: &'static str,
    offset: Vec3,
    rotation: f32,
    anchor: Vec2,
    scale: Vec2,
    flip_x: bool,
    flip_y: bool,
    color_kind: ObjectColorKind,
    opacity: f32,
    children: &'static [ObjectChild],
}

impl Default for ObjectChild {
    fn default() -> Self {
        ObjectChild {
            texture: "emptyFrame.png",
            offset: Vec3::ZERO,
            rotation: 0.,
            anchor: Vec2::ZERO,
            scale: Vec2::ONE,
            flip_x: false,
            flip_y: false,
            color_kind: ObjectColorKind::None,
            opacity: 1.,
            children: &[],
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/generated_object.rs"));

#[derive(Clone, Component, Default)]
pub(crate) struct Object {
    pub(crate) id: u64,
    frame_name: String,
    flip_x: bool,
    flip_y: bool,
    pub(crate) z_layer: i32,
}

pub(crate) fn spawn_object(
    commands: &mut World,
    object_data: &HashMap<&[u8], &[u8]>,
    global_sections: &GlobalSections,
) -> Result<Entity, anyhow::Error> {
    let mut object = Object::default();
    let mut object_color = ObjectColor::default();
    let mut transform = Transform::default();

    if let Some(id) = object_data.get(b"1".as_ref()) {
        object.id = std::str::from_utf8(id)?.parse()?;
    }

    let object_default_data = OBJECT_DEFAULT_DATA
        .get(&object.id)
        .unwrap_or(&ObjectDefaultData::DEFAULT);

    if let Some(x) = object_data.get(b"2".as_ref()) {
        transform.translation.x = std::str::from_utf8(x)?.parse()?;
    }
    if let Some(y) = object_data.get(b"3".as_ref()) {
        transform.translation.y = std::str::from_utf8(y)?.parse()?;
    }
    if let Some(rotation) = object_data.get(b"6".as_ref()) {
        transform.rotation =
            Quat::from_rotation_z(-std::str::from_utf8(rotation)?.parse::<f32>()?.to_radians());
    }
    if let Some(z_layer) = object_data.get(b"24".as_ref()) {
        object.z_layer = std::str::from_utf8(z_layer)?.parse()?;
    } else {
        object.z_layer = object_default_data.default_z_layer;
    }
    if let Some(z_order) = object_data.get(b"25".as_ref()) {
        transform.translation.z = std::str::from_utf8(z_order)?.parse()?;
    } else {
        transform.translation.z = object_default_data.default_z_order as f32;
    }
    transform.translation.z = (transform.translation.z + 999.) / (999. + 10000.) * 999.;
    if let Some(scale) = object_data.get(b"32".as_ref()) {
        transform.scale = Vec2::splat(std::str::from_utf8(scale)?.parse()?).extend(0.);
    }
    if let Some(flip_x) = object_data.get(b"4".as_ref()) {
        transform.scale.x *= if u8_to_bool(flip_x) { -1. } else { 1. };
    }
    if let Some(flip_y) = object_data.get(b"5".as_ref()) {
        transform.scale.y *= if u8_to_bool(flip_y) { -1. } else { 1. };
    }

    let mut base_color_channel = if let Some(base_color_channel) = object_data.get(b"21".as_ref()) {
        std::str::from_utf8(base_color_channel)?.parse()?
    } else {
        object_default_data.default_base_color_channel
    };
    let mut detail_color_channel =
        if let Some(detail_color_channel) = object_data.get(b"22".as_ref()) {
            std::str::from_utf8(detail_color_channel)?.parse()?
        } else {
            object_default_data.default_detail_color_channel
        };

    let mut base_hsv = if let Some(base_hsv) = object_data.get(b"43".as_ref()) {
        Some(HsvMod::parse(base_hsv)?)
    } else {
        None
    };

    let mut detail_hsv = if let Some(detail_hsv) = object_data.get(b"44".as_ref()) {
        Some(HsvMod::parse(detail_hsv)?)
    } else {
        None
    };

    if object_default_data.swap_base_detail {
        std::mem::swap(&mut base_color_channel, &mut detail_color_channel);
        std::mem::swap(&mut base_hsv, &mut detail_hsv);
    }

    match object_default_data.color_kind {
        ObjectColorKind::Base => {
            object_color.channel_id = base_color_channel;
            object_color.hsv = base_hsv;
        }
        ObjectColorKind::Detail => {
            object_color.channel_id = detail_color_channel;
            object_color.hsv = detail_hsv;
        }
        ObjectColorKind::Black => {
            object_color.channel_id = if base_color_channel != u64::MAX {
                base_color_channel
            } else {
                detail_color_channel
            };
        }
        ObjectColorKind::None => {}
    }

    object_color.opacity = object_default_data.opacity;

    let object_id = object.id;
    let object_z_layer = object.z_layer;
    let mut entity = commands
        .spawn(object)
        .insert(transform)
        .insert(GlobalTransform::default())
        .insert(object_color)
        .insert(object_default_data.color_kind)
        .id();

    global_sections
        .0
        .entry(SectionIndex::from_pos(transform.translation.xy()))
        .or_default()
        .insert(entity);

    Ok(entity)
}
