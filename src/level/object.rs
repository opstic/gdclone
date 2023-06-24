use crate::level::color::Hsv;
use crate::level::{trigger, Sections};
use crate::loaders::cocos2d_atlas::{Cocos2dAtlas, Cocos2dAtlasSprite, Cocos2dFrames};
use crate::utils::{section_from_pos, u8_to_bool};
use bevy::asset::Assets;
use bevy::hierarchy::{BuildChildren, Children, Parent};
use bevy::math::{IVec2, Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{
    Commands, Component, Entity, GlobalTransform, OrthographicProjection, Query, Res, Transform,
    With, Without,
};
use bevy::reflect::Reflect;
use bevy::render::view::VisibleEntities;
use bevy::sprite::Anchor;
use bevy::utils::{default, HashMap, HashSet};

#[derive(Component, Default, Reflect)]
pub(crate) struct Object {
    pub(crate) id: u64,
    pub(crate) z_layer: i8,
    pub(crate) color_channel: u64,
    #[reflect(ignore)]
    pub(crate) hsv: Option<Hsv>,
    pub(crate) groups: Vec<u64>,
    pub(crate) opacity: f32,
    pub(crate) black: bool,
}

pub(crate) fn update_visibility(
    mut camera_query: Query<(
        &mut VisibleEntities,
        &OrthographicProjection,
        &GlobalTransform,
    )>,
    sections: Res<Sections>,
) {
    for (mut visible_entities, projection, camera_transform) in &mut camera_query {
        let camera_min = projection.area.min + camera_transform.translation().xy();
        let camera_max = projection.area.max + camera_transform.translation().xy();
        let min_section = section_from_pos(camera_min);
        let max_section = section_from_pos(camera_max);

        let x_range = min_section.x - 1..max_section.x + 2;
        let y_range = min_section.y - 1..max_section.y + 2;

        for section_x in x_range {
            for section_y in y_range.clone() {
                if let Some(section) = sections.get_section(&IVec2::new(section_x, section_y)) {
                    visible_entities.entities.extend(section);
                }
            }
        }
    }
}

pub(crate) fn propagate_visibility(
    root_query: Query<&Children, (With<Cocos2dAtlasSprite>, Without<Parent>)>,
    children_query: Query<&Children, With<Cocos2dAtlasSprite>>,
    mut visible_entities_query: Query<&mut VisibleEntities>,
) {
    let mut children_of_child = HashSet::new();
    for mut visible_entities in &mut visible_entities_query {
        for children in root_query.iter_many(&visible_entities.entities) {
            for child in children {
                children_of_child.extend(recursive_get_child(child, &children_query));
            }
        }
        visible_entities
            .entities
            .extend(std::mem::take(&mut children_of_child));
    }
}

fn recursive_get_child(
    child: &Entity,
    children_query: &Query<&Children, With<Cocos2dAtlasSprite>>,
) -> HashSet<Entity> {
    let mut children_of_child = HashSet::new();
    if let Ok(children) = children_query.get(*child) {
        for child in children {
            children_of_child.extend(recursive_get_child(child, children_query));
        }
    }
    children_of_child.insert(*child);
    children_of_child
}

enum ObjectColorType {
    Base,
    Detail,
    Black,
    None,
}

struct ObjectDefaultData {
    texture: String,
    default_z_layer: i8,
    default_z_order: i16,
    default_base_color_channel: u64,
    default_detail_color_channel: u64,
    color_type: ObjectColorType,
    opacity: f32,
    children: Vec<ObjectChild>,
}

impl Default for ObjectDefaultData {
    fn default() -> Self {
        ObjectDefaultData {
            texture: "emptyFrame.png".to_string(),
            default_z_layer: 0,
            default_z_order: 0,
            default_base_color_channel: u64::MAX,
            default_detail_color_channel: u64::MAX,
            color_type: ObjectColorType::None,
            opacity: 1.,
            children: Vec::new(),
        }
    }
}

struct ObjectChild {
    texture: String,
    offset: Vec3,
    rotation: f32,
    anchor: Vec2,
    scale: Vec2,
    flip_x: bool,
    flip_y: bool,
    color_type: ObjectColorType,
    opacity: f32,
    children: Vec<ObjectChild>,
}

impl Default for ObjectChild {
    fn default() -> Self {
        ObjectChild {
            texture: "emptyFrame.png".to_string(),
            offset: Vec3::ZERO,
            rotation: 0.,
            anchor: Vec2::ZERO,
            scale: Vec2::ONE,
            flip_x: false,
            flip_y: false,
            color_type: ObjectColorType::None,
            opacity: 1.,
            children: Vec::new(),
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/generated_object.rs"));

pub(crate) fn spawn_object(
    commands: &mut Commands,
    object_data: &HashMap<&[u8], &[u8]>,
    groups: Vec<u64>,
    sections: &mut Sections,
    cocos2d_frames: &Cocos2dFrames,
    cocos2d_atlases: &Assets<Cocos2dAtlas>,
) -> Result<Entity, anyhow::Error> {
    let mut object = Object::default();
    let mut transform = Transform::default();
    let mut sprite = Cocos2dAtlasSprite::default();

    if let Some(id) = object_data.get(b"1".as_ref()) {
        object.id = std::str::from_utf8(id)?.parse()?;
    }

    let object_default_data: ObjectDefaultData = object_handler(object.id);

    sprite.texture = object_default_data.texture.clone();

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

    let base_color_channel = if let Some(base_color_channel) = object_data.get(b"21".as_ref()) {
        std::str::from_utf8(base_color_channel)?.parse()?
    } else {
        object_default_data.default_base_color_channel
    };
    let detail_color_channel = if let Some(detail_color_channel) = object_data.get(b"22".as_ref()) {
        std::str::from_utf8(detail_color_channel)?.parse()?
    } else {
        object_default_data.default_detail_color_channel
    };

    let base_hsv = if let Some(base_hsv) = object_data.get(b"43".as_ref()) {
        Some(Hsv::parse(base_hsv)?)
    } else {
        None
    };

    let detail_hsv = if let Some(detail_hsv) = object_data.get(b"44".as_ref()) {
        Some(Hsv::parse(detail_hsv)?)
    } else {
        None
    };

    match object_default_data.color_type {
        ObjectColorType::Base => {
            object.color_channel = base_color_channel;
            object.hsv = base_hsv;
        }
        ObjectColorType::Detail => {
            object.color_channel = detail_color_channel;
            object.hsv = detail_hsv;
        }
        ObjectColorType::Black => {
            object.color_channel = if base_color_channel != u64::MAX {
                base_color_channel
            } else {
                detail_color_channel
            };
            object.black = true;
        }
        ObjectColorType::None => {}
    }

    object.opacity = object_default_data.opacity;

    let object_id = object.id;
    let object_z_layer = object.z_layer;
    object.groups = groups.clone();
    let mut entity = commands.spawn(object);

    entity.insert(transform);
    entity.insert(GlobalTransform::default());
    entity.insert(sprite);
    if let Some((_, handle)) = cocos2d_frames.frames.get(&object_default_data.texture) {
        let mut handle = handle.clone();
        handle.make_strong(cocos2d_atlases);
        entity.insert(handle);
    }
    let entity = entity.id();
    let section = section_from_pos(transform.translation.xy());
    sections.get_section_mut(&section).insert(entity);
    match object_id {
        901 | 1007 | 1346 | 1049 | 899 => {
            trigger::setup_trigger(commands, entity, &object_id, object_data)?
        }
        _ => (),
    }

    for child in object_default_data.children {
        let child_entity = recursive_spawn_child(
            commands,
            child,
            base_color_channel,
            detail_color_channel,
            base_hsv,
            detail_hsv,
            object_z_layer,
            groups.clone(),
            cocos2d_frames,
            cocos2d_atlases,
        );
        commands.entity(entity).add_child(child_entity);
    }
    Ok(entity)
}

fn recursive_spawn_child(
    commands: &mut Commands,
    child: ObjectChild,
    base_color_channel: u64,
    detail_color_channel: u64,
    base_hsv: Option<Hsv>,
    detail_hsv: Option<Hsv>,
    z_layer: i8,
    groups: Vec<u64>,
    cocos2d_frames: &Cocos2dFrames,
    cocos2d_atlases: &Assets<Cocos2dAtlas>,
) -> Entity {
    let mut object = Object {
        groups: groups.clone(),
        z_layer,
        ..default()
    };

    match child.color_type {
        ObjectColorType::Base => {
            object.color_channel = base_color_channel;
            object.hsv = base_hsv;
        }
        ObjectColorType::Detail => {
            object.color_channel = detail_color_channel;
            object.hsv = detail_hsv;
        }
        ObjectColorType::Black => {
            object.color_channel = if base_color_channel != u64::MAX {
                base_color_channel
            } else {
                detail_color_channel
            };
            object.black = true;
        }
        ObjectColorType::None => {}
    }

    object.opacity = child.opacity;

    let mut entity = commands.spawn(object);
    entity.insert(GlobalTransform::default());
    let flip = Vec2::new(
        if child.flip_x { -1. } else { 1. },
        if child.flip_y { -1. } else { 1. },
    );
    entity.insert(Transform {
        translation: child.offset.xy().extend(child.offset.z / 999.),
        rotation: Quat::from_rotation_z(child.rotation.to_radians()),
        scale: (child.scale * flip).extend(0.),
    });
    entity.insert(Cocos2dAtlasSprite {
        texture: child.texture.clone(),
        anchor: Anchor::Custom(child.anchor * 2. * flip),
        ..default()
    });
    if let Some((_, handle)) = cocos2d_frames.frames.get(&child.texture) {
        let mut handle = handle.clone();
        handle.make_strong(cocos2d_atlases);
        entity.insert(handle);
    }
    let entity = entity.id();
    for child in child.children {
        let child_entity = recursive_spawn_child(
            commands,
            child,
            base_color_channel,
            detail_color_channel,
            base_hsv,
            detail_hsv,
            z_layer,
            groups.clone(),
            cocos2d_frames,
            cocos2d_atlases,
        );
        commands.entity(entity).add_child(child_entity);
    }
    entity
}
