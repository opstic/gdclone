use crate::level::color::Hsv;
use crate::level::trigger;
use crate::level::trigger::XPosActivate;
use crate::loaders::cocos2d_atlas::{find_texture, Cocos2dAtlas};
use crate::states::loading::GlobalAssets;
use crate::utils::u8_to_bool;
use bevy::asset::Assets;
use bevy::hierarchy::{BuildChildren, Parent};
use bevy::log::info;
use bevy::math::{Quat, Rect, Vec2, Vec3, Vec3Swizzles, Vec4};
use bevy::prelude::{
    Camera, Camera2d, Commands, Component, ComputedVisibility, Entity, GlobalTransform, Local,
    OrthographicProjection, Query, Res, Transform, Visibility, With, Without,
};
use bevy::render::primitives::{Aabb, Frustum};
use bevy::render::view::{NoFrustumCulling, RenderLayers, VisibleEntities};
use bevy::sprite::{Anchor, SpriteSheetBundle, TextureAtlasSprite};
use bevy::utils::{default, HashMap};
use std::cell::Cell;
use std::cmp::{max, min};
use std::ops::Index;
use thread_local::ThreadLocal;

#[derive(Component, Default)]
pub(crate) struct Object {
    pub(crate) id: u64,
    pub(crate) z_layer: i8,
    pub(crate) color_channel: u64,
    pub(crate) hsv: Option<Hsv>,
    pub(crate) rotated: bool,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) transform: Transform,
    pub(crate) groups: Vec<u64>,
    pub(crate) texture_name: String,
    pub(crate) additional_anchor: Vec2,
}

#[derive(Component)]
pub(crate) struct ObjectVisibility {
    pub(crate) visible: bool,
}

pub(crate) fn update_visibility(
    mut thread_queues: Local<ThreadLocal<Cell<Vec<Entity>>>>,
    mut projection_query: Query<(
        &mut VisibleEntities,
        &OrthographicProjection,
        &GlobalTransform,
    )>,
    mut objects: Query<(Entity, &mut ObjectVisibility, &Aabb, &GlobalTransform)>,
) {
    for (mut visible_entities, projection, camera_transform) in &mut projection_query {
        let camera_min = camera_transform.transform_point(projection.area.min.extend(0.));
        let camera_max = camera_transform.transform_point(projection.area.max.extend(0.));
        let camera_bounds = Vec4::new(camera_min.x, camera_min.y, -camera_max.x, -camera_max.y);
        objects
            .par_iter_mut()
            .for_each_mut(|(entity, mut visibility, aabb, transform)| {
                let min = transform.transform_point(aabb.min().into()).xy();
                let max = transform.transform_point(aabb.max().into()).xy();
                let object_bounds = Vec4::new(max.x, max.y, -min.x, -min.y);
                // let compare_results = camera_bounds.cmple(object_bounds);
                // info!("{:?}", compare_results);
                // let compare_results = compare_results.bitmask();
                // if compare_results == 0x00ff || compare_results == 0xff00 {
                if !camera_bounds.cmple(object_bounds).all() {
                    visibility.visible = false;
                    return;
                }
                visibility.visible = true;
                let cell = thread_queues.get_or_default();
                let mut queue = cell.take();
                queue.push(entity);
                cell.set(queue);
            });
        for cell in thread_queues.iter_mut() {
            visible_entities.entities.append(cell.get_mut());
        }
    }
}

struct ObjectDefaultData {
    texture_name: String,
    default_z_layer: i8,
    default_z_order: i16,
    childrens: Vec<Children>,
}

impl Default for ObjectDefaultData {
    fn default() -> Self {
        ObjectDefaultData {
            texture_name: "emptyFrame.png".to_string(),
            default_z_layer: 0,
            default_z_order: 0,
            childrens: Vec::new(),
        }
    }
}

#[derive(Default)]
struct Children {
    texture_name: String,
    x: f32,
    y: f32,
    z: i16,
    rot: f32,
}

include!(concat!(env!("OUT_DIR"), "/generated_object.rs"));

pub(crate) fn spawn_object(
    commands: &mut Commands,
    object_data: &HashMap<&[u8], &[u8]>,
    groups: Vec<u64>,
) -> Result<Entity, anyhow::Error> {
    let mut object = Object::default();
    if let Some(id) = object_data.get(b"1".as_ref()) {
        object.id = std::str::from_utf8(id)?.parse()?;
    }

    let object_default_data: ObjectDefaultData = object_handler(object.id);

    if let Some(x) = object_data.get(b"2".as_ref()) {
        object.transform.translation.x = std::str::from_utf8(x)?.parse()?;
    }
    if let Some(y) = object_data.get(b"3".as_ref()) {
        object.transform.translation.y = std::str::from_utf8(y)?.parse()?;
    }
    if let Some(flip_x) = object_data.get(b"4".as_ref()) {
        object.flip_x = u8_to_bool(flip_x);
    }
    if let Some(flip_y) = object_data.get(b"5".as_ref()) {
        object.flip_y = u8_to_bool(flip_y);
    }
    if let Some(rotation) = object_data.get(b"6".as_ref()) {
        object.transform.rotation =
            Quat::from_rotation_z(std::str::from_utf8(rotation)?.parse::<f32>()?.to_radians());
    }
    if let Some(z_layer) = object_data.get(b"24".as_ref()) {
        object.z_layer = std::str::from_utf8(z_layer)?.parse()?;
    } else {
        object.z_layer = object_default_data.default_z_layer;
    }
    if let Some(z_order) = object_data.get(b"25".as_ref()) {
        object.transform.translation.z = std::str::from_utf8(z_order)?.parse()?;
    } else {
        object.transform.translation.z = object_default_data.default_z_order as f32;
    }
    if let Some(scale) = object_data.get(b"32".as_ref()) {
        object.transform.scale = Vec2::splat(std::str::from_utf8(scale)?.parse()?).extend(0.);
    }
    if let Some(color_channel) = object_data.get(b"21".as_ref()) {
        object.color_channel = std::str::from_utf8(color_channel)?.parse()?;
    }
    if let Some(hsv) = object_data.get(b"43".as_ref()) {
        object.hsv = Some(Hsv::parse(hsv)?);
    }
    let object_id = object.id;
    let object_z_layer = object.z_layer;
    let object_color_channel = object.color_channel;
    let object_hsv = object.hsv.clone();
    object.groups = groups.clone();
    object.texture_name = object_default_data.texture_name;
    let entity = commands.spawn(object).id();
    match object_id {
        901 | 1007 | 1346 | 1049 | 899 => {
            trigger::setup_trigger(commands, entity, &object_id, object_data)?
        }
        _ => (),
    }
    // for child in object_default_data.childrens {
    //     let mut child_object = Object {
    //         texture_name: child.texture_name,
    //         z_layer: object_z_layer,
    //         transform: Transform {
    //             translation: child.offset,
    //             rotation: Quat::from_rotation_z(child.rot.to_radians()),
    //             ..default()
    //         },
    //         ..default()
    //     };
    //     if let Some(secondary_color_channel) = object_data.get(b"22".as_ref()) {
    //         child_object.color_channel = std::str::from_utf8(secondary_color_channel)?.parse()?;
    //     } else {
    //         child_object.color_channel = object_color_channel;
    //     }
    //     if let Some(secondary_hsv) = object_data.get(b"44".as_ref()) {
    //         child_object.hsv = Some(Hsv::parse(secondary_hsv)?);
    //     } else {
    //         child_object.hsv = object_hsv.clone();
    //     }
    //     child_object.groups = groups.clone();
    //     child_object.additional_anchor = child.anchor;
    //     child_object.flip_x = child.flip_x;
    //     child_object.flip_y = child.flip_y;
    //     let child_entity = commands.spawn(child_object).id();
    //     commands.entity(entity).add_child(child_entity);
    // }
    Ok(entity)
}

pub(crate) fn create_sprite(
    mut commands: Commands,
    global_assets: Res<GlobalAssets>,
    cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
    object_without_sprite: Query<(Entity, &Object), (Without<TextureAtlasSprite>, Without<Parent>)>,
) {
    let atlases = vec![
        &global_assets.atlas1,
        &global_assets.atlas2,
        &global_assets.atlas3,
        &global_assets.atlas4,
        &global_assets.atlas5,
    ];
    // object_without_sprite
    //     .par_iter()
    //     .for_each_mut(|(entity, object)| {});
    for (entity, object) in object_without_sprite.iter() {
        if let Some((info, atlas_handle)) =
            find_texture(&cocos2d_atlases, &atlases, &object.texture_name)
        {
            let mut flip_x = object.flip_x;
            let mut flip_y = object.flip_y;
            let translation = (object.transform.translation.xy() * 4.)
                .extend((object.transform.translation.z + 999.) / (999. + 10000.) * 999.);
            let mut rotation = object.transform.rotation;
            if info.rotated {
                std::mem::swap(&mut flip_x, &mut flip_y);
                rotation *= Quat::from_rotation_z((-90_f32).to_radians())
            }
            rotation = rotation.inverse();
            let mut scale = object.transform.scale;
            scale.x *= if flip_x { -1. } else { 1. };
            scale.y *= if flip_y { -1. } else { 1. };
            let mut entity = commands.entity(entity);
            entity.insert(SpriteSheetBundle {
                transform: Transform {
                    translation,
                    rotation,
                    scale,
                },
                sprite: TextureAtlasSprite {
                    index: info.index,
                    anchor: Anchor::Custom(info.anchor + object.additional_anchor),
                    ..default()
                },
                texture_atlas: atlas_handle.clone(),
                ..default()
            });
            entity.remove::<Visibility>();
            entity.remove::<ComputedVisibility>();
            entity.insert(ObjectVisibility { visible: false });
        }
    }
}

// pub(crate) fn create_child_sprite(mut commands: Commands, global_assets: Res<GlobalAssets>,
//                                   cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
//                                   object_without_sprite: Query<(Entity, &Object, &Visibility), (Without<TextureAtlasSprite>, With<Parent>)>)

pub(crate) fn check_visible(
    mut objects: Query<(&Transform, &Object, &mut Visibility)>,
    camera_transforms: Query<&Transform, (With<Camera2d>, Without<Object>)>,
) {
    let player_x = camera_transforms
        .get_single()
        .unwrap_or(&Transform::default())
        .translation
        .x;
    objects
        .par_iter_mut()
        .for_each_mut(|(transform, _, mut visibility)| {
            if transform.translation.x < player_x - 100.
                || transform.translation.x > player_x + 100.
            {
                *visibility = Visibility::Hidden;
            } else {
                *visibility = Visibility::Inherited;
            }
        })
}
