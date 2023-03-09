use crate::level::color;
use crate::level::color::{ColorChannels, Hsv};
use crate::loaders::cocos2d_atlas::{find_texture, Cocos2dAtlas, Cocos2dTextureInfo};
use crate::loaders::mapping::Mapping;
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
    pub(crate) z_layer: i8,
    pub(crate) color_channel: u64,
    pub(crate) hsv: Option<Hsv>,
    pub(crate) rotated: bool,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) transform: Transform,
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
    if let Some(z_layer) = object_data.get(b"24".as_ref()) {
        object.z_layer = std::str::from_utf8(z_layer)?.parse()?;
    }
    if let Some(z_order) = object_data.get(b"25".as_ref()) {
        object.transform.translation.z = std::str::from_utf8(z_order)?.parse()?;
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

pub(crate) fn create_sprite(
    mut commands: Commands,
    global_assets: Res<GlobalAssets>,
    mapping: Res<Assets<Mapping>>,
    cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
    object_without_cocos2d: Query<(Entity, &Object), Without<TextureAtlasSprite>>,
) {
    let atlases = vec![
        &global_assets.atlas1,
        &global_assets.atlas2,
        &global_assets.atlas3,
        &global_assets.atlas4,
        &global_assets.atlas5,
    ];
    for (entity, object) in object_without_cocos2d.iter() {
        if let Some((info, handle)) = find_texture(
            &mapping.get(&global_assets.texture_mapping).unwrap().mapping,
            &cocos2d_atlases,
            &atlases,
            &object.id,
        ) {
            let mut flip_x = object.flip_x;
            let mut flip_y = object.flip_y;
            let mut translation =
                (object.transform.translation.xy() * 4.).extend(object.transform.translation.z);
            let mut rotation = object.transform.rotation;
            if info.rotated {
                std::mem::swap(&mut flip_x, &mut flip_y);
                rotation *= Quat::from_rotation_z((-90 as f32).to_radians())
            }
            rotation = rotation.inverse();
            let mut entity = commands.entity(entity);
            entity.insert(SpriteSheetBundle {
                transform: Transform {
                    translation,
                    rotation,
                    scale: object.transform.scale,
                },
                sprite: TextureAtlasSprite {
                    index: info.index,
                    flip_x,
                    flip_y,
                    anchor: Anchor::Custom(info.anchor),
                    ..default()
                },
                texture_atlas: handle.clone(),
                ..default()
            });
        }
    }
}
