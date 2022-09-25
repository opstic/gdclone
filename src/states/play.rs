use crate::loaders::gdlevel::GDLevel;
use crate::{GameState, GlobalAssets, ObjectMapping, TexturePackerAtlas};
use bevy::prelude::*;
use iyes_loopless::prelude::{AppLooplessStateExt, ConditionSet, NextState};

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameState::PlayState, play_setup)
            .add_exit_system(GameState::PlayState, play_cleanup)
            .add_system_set(
                ConditionSet::new()
                    .run_in_state(GameState::PlayState)
                    .with_system(move_camera)
                    .with_system(exit_play)
                    .into(),
            );
    }
}

fn play_setup(
    mut commands: Commands,
    global_assets: Res<GlobalAssets>,
    level: Res<GDLevel>,
    mapping: Res<Assets<ObjectMapping>>,
    packer_atlases: Res<Assets<TexturePackerAtlas>>,
) {
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
            info!("Object not found in mapping: {:?}", object);
            break;
        }
        if let Some(handle) = atlas_handle {
            info!("{:?}", object.id);
            commands
                .spawn_bundle(SpriteSheetBundle {
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
                })
                .insert(LevelObject);
        } else {
            info!("Object texture not found: {:?}", object);
            break;
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
        commands.insert_resource(NextState(GameState::LevelSelectState));
    }
}

#[derive(Component)]
struct LevelObject;

fn play_cleanup(mut commands: Commands, query: Query<Entity, With<LevelObject>>) {
    query.for_each(|entity| commands.entity(entity).despawn_recursive());
}
