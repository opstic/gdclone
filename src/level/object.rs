use crate::level::color;
use crate::level::color::{ColorChannels, Hsv};
use crate::loaders::cocos2d_atlas::{Cocos2dAtlas, Cocos2dAtlasSprite};
use crate::loaders::mapping::Mapping;
use crate::render::sprite::BlendingSprite;
use crate::states::loading::GlobalAssets;
use crate::utils::u8_to_bool;
use bevy::asset::{Assets, Handle};
use bevy::math::{Quat, Vec2, Vec3Swizzles};
use bevy::prelude::{Commands, Component, Entity, Query, Res, Transform, Without};
use bevy::sprite::{Anchor, SpriteSheetBundle, TextureAtlas, TextureAtlasSprite};
use bevy::utils::{default, HashMap};

#[derive(Component, Default)]
pub(crate) struct Object {
    pub(crate) id: u64,
    pub(crate) transform: Transform,
    pub(crate) z_layer: u8,
    pub(crate) z_order: u16,
    pub(crate) color_channel: u64,
    pub(crate) hsv: Option<Hsv>,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
}

pub(crate) fn spawn_object(
    commands: &mut Commands,
    object_data: &HashMap<&[u8], &[u8]>,
) -> Result<Entity, anyhow::Error> {
    let mut object = Object::default();
    if let Some(id) = object_data.get(b"1".as_ref()) {
        object.id = std::str::from_utf8(id)?.parse()?;
    }
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
    if let Some(scale) = object_data.get(b"32".as_ref()) {
        object.transform.scale =
            (std::str::from_utf8(scale)?.parse::<f32>()? * Vec2::ONE).extend(0.);
    }
    if let Some(color_channel) = object_data.get(b"21".as_ref()) {
        object.color_channel = std::str::from_utf8(color_channel)?.parse()?;
    }
    if let Some(hsv) = object_data.get(b"43".as_ref()) {
        object.hsv = Some(Hsv::parse(hsv)?);
    }
    Ok(commands.spawn(object).id())
}

pub(crate) fn create_atlas_sprite(
    mut commands: Commands,
    global_assets: Res<GlobalAssets>,
    mapping: Res<Assets<Mapping>>,
    cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
    object_without_cocos2d: Query<(Entity, &Object), Without<Cocos2dAtlasSprite>>,
) {
    let atlases = vec![
        &global_assets.atlas1,
        &global_assets.atlas2,
        &global_assets.atlas3,
        &global_assets.atlas4,
        &global_assets.atlas5,
    ];
    for (entity, object) in object_without_cocos2d.iter() {
        if let Some((index, anchor, rotated, handle)) = find_texture(
            &mapping.get(&global_assets.texture_mapping).unwrap().mapping,
            &cocos2d_atlases,
            &atlases,
            &object.id,
        ) {
            commands.entity(entity).insert(Cocos2dAtlasSprite {
                index,
                anchor,
                rotated,
                handle,
            });
        }
    }
}

pub(crate) fn create_sprite(
    mut commands: Commands,
    colors: Res<ColorChannels>,
    cocos2d_without_sprite: Query<
        (Entity, &Object, &Cocos2dAtlasSprite),
        Without<TextureAtlasSprite>,
    >,
) {
    for (entity, object, sprite) in cocos2d_without_sprite.iter() {
        let (color, blending) = color::get_color(&colors.0, &object.color_channel);
        let mut flip_x = object.flip_x;
        let mut flip_y = object.flip_y;
        let mut translation =
            (object.transform.translation.xy() * 4.).extend(object.z_order as f32);
        let mut rotation = object.transform.rotation;
        if sprite.rotated {
            std::mem::swap(&mut flip_x, &mut flip_y);
            rotation *= Quat::from_rotation_z((-90 as f32).to_radians())
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
                color,
                index: sprite.index,
                anchor: Anchor::Custom(sprite.anchor),
                ..default()
            },
            texture_atlas: sprite.handle.clone(),
            ..default()
        });
        if blending {
            entity.insert(BlendingSprite);
        }
    }
}

#[inline(always)]
fn find_texture(
    mapping: &HashMap<u64, String>,
    cocos2d_atlases: &Res<Assets<Cocos2dAtlas>>,
    atlases: &Vec<&Handle<Cocos2dAtlas>>,
    id: &u64,
) -> Option<(usize, Vec2, bool, Handle<TextureAtlas>)> {
    let texture_name = mapping.get(&*id);
    if let Some(name) = texture_name {
        for atlas_handle in atlases {
            if let Some(atlas) = cocos2d_atlases.get(atlas_handle) {
                if let Some((index, anchor, rotated)) = atlas.index.get(name) {
                    return Some((*index, *anchor, *rotated, atlas.texture_atlas.clone()));
                }
            }
        }
        None
    } else {
        None
    }
}
