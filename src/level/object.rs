use bevy::asset::Handle;
use bevy::hierarchy::BuildWorldChildren;
use bevy::log::debug;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{Component, Entity, World};
use bevy::utils::{default, HashMap};
use indexmap::{IndexMap, IndexSet};

use crate::asset::cocos2d_atlas::{Cocos2dFrame, Cocos2dFrames};
use crate::level::color::{GlobalColorChannels, HsvMod, ObjectColorCalculated};
use crate::level::color::{ObjectColor, ObjectColorKind};
use crate::level::de;
use crate::level::group::ObjectGroupsCalculated;
use crate::level::section::{GlobalSections, Section, SectionIndex};
use crate::level::transform::{GlobalTransform2d, Transform2d};
use crate::level::trigger::insert_trigger_data;
use crate::utils::{u8_to_bool, U64Hash};

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
    pub(crate) frame: Cocos2dFrame,
    pub(crate) anchor: Vec2,
    pub(crate) z_layer: i32,
}

pub(crate) fn get_object_pos(object_data: &HashMap<&[u8], &[u8]>) -> Result<Vec2, anyhow::Error> {
    let mut translation = Vec2::ZERO;
    if let Some(x) = object_data.get(b"2".as_ref()) {
        translation.x = std::str::from_utf8(x)?.parse()?;
    }
    if let Some(y) = object_data.get(b"3".as_ref()) {
        translation.y = std::str::from_utf8(y)?.parse()?;
    }
    Ok(translation)
}

pub(crate) fn spawn_object(
    world: &mut World,
    object_data: &HashMap<&[u8], &[u8]>,
    global_sections: &GlobalSections,
    global_groups: &mut IndexMap<u64, Vec<Entity>, U64Hash>,
    global_color_channels: &GlobalColorChannels,
    cocos2d_frames: &Cocos2dFrames,
) -> Result<Entity, anyhow::Error> {
    let mut object = Object::default();
    let mut object_color = ObjectColor::default();
    let mut transform = Transform2d::default();

    if let Some(id) = object_data.get(b"1".as_ref()) {
        object.id = std::str::from_utf8(id)?.parse()?;
    }

    let object_default_data = OBJECT_DEFAULT_DATA
        .get(&object.id)
        .unwrap_or(&ObjectDefaultData::DEFAULT);

    object_color.object_opacity = object_default_data.opacity;
    object_color.object_color_kind = object_default_data.color_kind;

    if let Some(x) = object_data.get(b"2".as_ref()) {
        transform.translation.x = std::str::from_utf8(x)?.parse()?;
    }
    if let Some(y) = object_data.get(b"3".as_ref()) {
        transform.translation.y = std::str::from_utf8(y)?.parse()?;
    }
    if let Some(rotation) = object_data.get(b"6".as_ref()) {
        transform.angle = -std::str::from_utf8(rotation)?.parse::<f32>()?.to_radians();
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
    if let Some(scale) = object_data.get(b"32".as_ref()) {
        transform.scale = Vec2::splat(std::str::from_utf8(scale)?.parse()?);
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

    if let Some(color_channel_entity) = global_color_channels.0.get(&object_color.channel_id) {
        object_color.channel_entity = *color_channel_entity;
    }

    let frame_index =
        if let Some(frame_index) = cocos2d_frames.index.get(object_default_data.texture) {
            frame_index
        } else {
            debug!(
            "Object {}: Cannot find texture with name \"{}\". Using \"emptyFrame.png\" instead.",
            object.id, object_default_data.texture
        );
            cocos2d_frames.index.get("emptyFrame.png").unwrap()
        };

    let (frame, image_asset_id) = &cocos2d_frames.frames[*frame_index];

    object.frame = *frame;

    let object_id = object.id;
    let object_z_layer = object.z_layer;
    let object_transform = GlobalTransform2d::from(transform);
    let mut entity = world.spawn((
        object,
        object_color,
        ObjectColorCalculated::default(),
        Section::from_section_index(SectionIndex::from_pos(transform.translation.xy())),
        transform,
        object_transform,
        Handle::Weak(*image_asset_id),
        ObjectGroupsCalculated::default(),
    ));

    insert_trigger_data(&mut entity, object_id, object_data)?;

    let entity = entity.id();

    let mut global_section = global_sections
        .sections
        .entry(SectionIndex::from_pos(transform.translation.xy()))
        .or_default();

    global_section.insert(entity);

    let groups: Vec<u64> = if let Some(group_string) = object_data.get(b"57".as_ref()) {
        de::from_slice(group_string, b'.')?
    } else {
        Vec::new()
    };

    let mut spawned = Vec::new();

    recursive_spawn_children(
        world,
        object_id,
        object_default_data.children,
        base_color_channel,
        detail_color_channel,
        base_hsv,
        detail_hsv,
        object_z_layer,
        &mut global_section,
        global_color_channels,
        cocos2d_frames,
        entity,
        &object_transform,
        &mut spawned,
    )?;

    for group in groups {
        let global_group = global_groups.entry(group).or_default();
        global_group.push(entity);
        global_group.extend(&spawned)
    }

    Ok(entity)
}

fn recursive_spawn_children(
    world: &mut World,
    object_id: u64,
    children: &[ObjectChild],
    base_color_channel: u64,
    detail_color_channel: u64,
    base_hsv: Option<HsvMod>,
    detail_hsv: Option<HsvMod>,
    z_layer: i32,
    global_section: &mut IndexSet<Entity, U64Hash>,
    global_color_channels: &GlobalColorChannels,
    cocos2d_frames: &Cocos2dFrames,
    parent_entity: Entity,
    parent_transform: &GlobalTransform2d,
    spawned: &mut Vec<Entity>,
) -> Result<(), anyhow::Error> {
    for child in children {
        let mut object = Object {
            z_layer,
            ..default()
        };
        let mut object_color = ObjectColor::default();

        match child.color_kind {
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

        object_color.object_opacity = child.opacity;
        object_color.object_color_kind = child.color_kind;

        let flip = Vec2::new(
            if child.flip_x { -1. } else { 1. },
            if child.flip_y { -1. } else { 1. },
        );
        let transform = Transform2d {
            translation: child.offset.xy().extend(child.offset.z / 1000.),
            angle: child.rotation.to_radians(),
            scale: child.scale * flip,
            ..default()
        };

        object.anchor = child.anchor * flip;

        if let Some(color_channel_entity) = global_color_channels.0.get(&object_color.channel_id) {
            object_color.channel_entity = *color_channel_entity;
        }

        let Some(frame_index) = cocos2d_frames.index.get(child.texture) else {
            debug!(
                "Object {}: Cannot find texture with name \"{}\". Skipping child.",
                object_id, child.texture
            );
            continue;
        };

        let (frame, image_asset_id) = &cocos2d_frames.frames[*frame_index];

        object.frame = *frame;

        let child_transform = parent_transform.mul_transform(transform);

        let child_entity = world
            .spawn((
                object,
                object_color,
                ObjectColorCalculated::default(),
                Section::default(),
                transform,
                child_transform,
                Handle::Weak(*image_asset_id),
                ObjectGroupsCalculated::default(),
            ))
            .id();

        world.entity_mut(parent_entity).add_child(child_entity);

        global_section.insert(child_entity);

        spawned.push(child_entity);

        recursive_spawn_children(
            world,
            object_id,
            child.children,
            base_color_channel,
            detail_color_channel,
            base_hsv,
            detail_hsv,
            z_layer,
            global_section,
            global_color_channels,
            cocos2d_frames,
            child_entity,
            &child_transform,
            spawned,
        )?;
    }
    Ok(())
}
