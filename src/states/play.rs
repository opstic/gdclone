use crate::{GDLevel, GameStates, LevelAssets, ObjectMapping, TexturePackerAtlas};
use bevy::prelude::*;
use bevy::render::camera;
use iyes_loopless::prelude::{AppLooplessStateExt, ConditionSet};
use std::thread::spawn;

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameStates::PlayState, play_setup)
            .add_system_set(
                ConditionSet::new()
                    .run_in_state(GameStates::PlayState)
                    .with_system(move_camera)
                    .into(),
            );
    }
}

fn play_setup(
    mut commands: Commands,
    level_assets: Res<LevelAssets>,
    levels: Res<Assets<GDLevel>>,
    mapping: Res<Assets<ObjectMapping>>,
    packer_atlases: Res<Assets<TexturePackerAtlas>>,
    texture_atlases: Res<Assets<TextureAtlas>>,
) {
    // let pack_atlases = packer_atlases.get(&level_assets.atlas1).unwrap();
    // let mut I = 0;
    //
    // for texture in pack_atlases.index.clone() {
    //     commands.spawn_bundle(SpriteSheetBundle {
    //         transform: Transform {
    //             translation: Vec3::from(((150*I) as f32, 0.0, 0.0)),
    //             ..default()
    //         },
    //         sprite: TextureAtlasSprite {
    //             index: I,
    //             ..default()
    //         },
    //         texture_atlas: pack_atlases.texture_atlas.clone(),
    //         ..default()
    //     });
    //     I += 1;
    //     }
    //
    //
    if let Some(level) = levels.get(&level_assets.level) {
        let mut i = 0;
        for object in &level.inner_level {
            let texture_name = mapping
                .get(&level_assets.texture_mapping)
                .unwrap()
                .mapping
                .get(&object.id);
            // let texture_name = Some("block001_slope_02_001.png");
            // for  in  {
            //
            // }
            info!("texture_name: {:?}", texture_name);
            let mut atlas_handle: Option<Handle<TextureAtlas>> = None;
            let mut atlas_mapping: usize = 0;
            let mut texture_rotated: bool = false;
            if let Some(name) = texture_name {
                let atlases = vec![
                    &level_assets.atlas1,
                    &level_assets.atlas2,
                    &level_assets.atlas3,
                    &level_assets.atlas4,
                    &level_assets.atlas5,
                ];
                for atlas in atlases {
                    let packer_atlas = packer_atlases.get(atlas).unwrap();
                    match packer_atlas.index.get(name) {
                        Some((mapping, rotated)) => {
                            atlas_handle = Some(packer_atlas.texture_atlas.clone());
                            atlas_mapping = mapping.clone();
                            texture_rotated = rotated.clone();
                            break;
                        }
                        None => continue,
                    }
                }
            } else {
                info!("Unknown object: {:?}", object);
                break;
            }
            if let Some(handle) = atlas_handle {
                info!("{:?}", object.id);
                commands.spawn_bundle(SpriteSheetBundle {
                    transform: Transform {
                        translation: Vec3::from((object.x, object.y, 0.)),
                        rotation: Quat::from_rotation_z(
                            -(object.rot + if texture_rotated { -90. } else { 0. }).to_radians(),
                        ),
                        scale: Vec3::new(object.scale, object.scale, 0.),
                    },
                    sprite: TextureAtlasSprite {
                        index: atlas_mapping,
                        flip_x: object.flip_x,
                        flip_y: object.flip_y,
                        ..Default::default()
                    },
                    texture_atlas: handle,
                    ..default()
                });
            } else {
                info!("Unknown object: {:?}", object);
                break;
            }
            i += 1;
        }
    }

    // commands.spawn_bundle(SpriteBundle {
    //     texture: texture_atlases
    //         .get(
    //             &packer_atlases
    //                 .get(&level_assets.atlas1)
    //                 .unwrap()
    //                 .texture_atlas,
    //         )
    //         .unwrap()
    //         .texture
    //         .clone(),
    //     transform: Transform::from_xyz(-300.0, 0.0, 0.0),
    //     ..default()
    // });
}

fn move_camera(
    mut camera_transforms: Query<&mut Transform, With<Camera>>,
    keys: Res<Input<KeyCode>>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
) {
    for mut transform in camera_transforms.iter_mut() {
        if keys.pressed(KeyCode::Right) {
            transform.translation.x += 10.0;
        }
        if keys.pressed(KeyCode::Left) {
            transform.translation.x -= 10.0;
        }
        if keys.pressed(KeyCode::Up) {
            transform.translation.y += 10.0;
        }
        if keys.pressed(KeyCode::Down) {
            transform.translation.y -= 10.0;
        }
        if keys.pressed(KeyCode::A) {
            transform.translation.x -= 30.0;
        }
        if keys.pressed(KeyCode::D) {
            transform.translation.x += 30.0;
        }
    }
    for mut projection in projections.iter_mut() {
        if keys.pressed(KeyCode::Q) {
            projection.scale *= 1.01;
        }
        if keys.pressed(KeyCode::E) {
            projection.scale *= 0.99;
        }
    }
}
