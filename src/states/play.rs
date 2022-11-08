use crate::{GDSaveFile, GameState, GlobalAssets, ObjectMapping, TexturePackerAtlas};
use bevy::prelude::*;
use iyes_loopless::prelude::{AppLooplessStateExt, ConditionSet, NextState};

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameState::Play, play_setup)
            .add_exit_system(GameState::Play, play_cleanup)
            .add_system_set(
                ConditionSet::new()
                    .run_in_state(GameState::Play)
                    .with_system(move_camera)
                    .with_system(exit_play)
                    .into(),
            );
    }
}

pub(crate) struct LevelIndex {
    pub(crate) index: usize,
}

fn play_setup(
    mut camera_transforms: Query<&mut Transform, With<Camera>>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
    mut commands: Commands,
    global_assets: Res<GlobalAssets>,
    save_file: Res<Assets<GDSaveFile>>,
    level_index: Res<LevelIndex>,
    mapping: Res<Assets<ObjectMapping>>,
    packer_atlases: Res<Assets<TexturePackerAtlas>>,
) {
    for mut transform in camera_transforms.iter_mut() {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
    }
    for mut projection in projections.iter_mut() {
        projection.scale = 1.0;
    }

    let level = &save_file
        .get(&global_assets.save_file)
        .unwrap()
        .levels
        .get(level_index.index)
        .unwrap();

    for object in &level.inner_level {
        let texture_name = mapping
            .get(&global_assets.texture_mapping)
            .unwrap()
            .mapping
            .get(&object.id);
        let mut atlas_handle: Option<Handle<TextureAtlas>> = None;
        let mut atlas_mapping: usize = 0;
        let mut texture_rotated: bool = false;
        if let Some(name) = texture_name {
            let atlases = vec![
                &global_assets.atlas1,
                &global_assets.atlas2,
                &global_assets.atlas3,
                &global_assets.atlas4,
                &global_assets.atlas5,
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
            warn!("Object not found in mapping: {:?}", object);
            continue;
        }
        if let Some(handle) = atlas_handle {
            commands
                .spawn_bundle(SpriteSheetBundle {
                    transform: Transform {
                        translation: Vec3::from((
                            object.x,
                            object.y,
                            (object.z_layer + 3) as f32 * 100.
                                + (object.z_order + 999) as f32 * 100. / (999. + 10000.)
                                - 0.099,
                        )),
                        rotation: Quat::from_rotation_z(
                            -(object.rot
                                + if texture_rotated { -90. } else { 0. }
                                + if texture_rotated && object.flip_x {
                                    180.
                                } else {
                                    0.
                                }
                                + if texture_rotated && object.flip_y {
                                    180.
                                } else {
                                    0.
                                })
                            .to_radians(),
                        ),
                        scale: Vec3::new(object.scale, object.scale, 0.),
                    },
                    sprite: TextureAtlasSprite {
                        index: atlas_mapping,
                        color: if let Some(color) =
                            level.start_object.colors.get(&object.main_color)
                        {
                            Color::rgba(
                                color.r as f32 / u8::MAX as f32,
                                color.g as f32 / u8::MAX as f32,
                                color.b as f32 / u8::MAX as f32,
                                color.opacity,
                            )
                        } else {
                            Color::WHITE
                        },
                        flip_x: object.flip_x,
                        flip_y: object.flip_y,
                        ..Default::default()
                    },
                    texture_atlas: handle,
                    ..default()
                })
                .insert(LevelObject);
        } else {
            warn!("Object texture not found: {:?}", object);
            continue;
        }
    }
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
        if keys.pressed(KeyCode::W) {
            transform.translation.y += 30.0;
        }
        if keys.pressed(KeyCode::S) {
            transform.translation.y -= 30.0;
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

fn exit_play(mut commands: Commands, keys: Res<Input<KeyCode>>) {
    if keys.pressed(KeyCode::Escape) {
        commands.insert_resource(NextState(GameState::LevelSelect));
    }
}

#[derive(Component)]
struct LevelObject;

fn play_cleanup(mut commands: Commands, query: Query<Entity, With<LevelObject>>) {
    query.for_each(|entity| commands.entity(entity).despawn_recursive());
}
