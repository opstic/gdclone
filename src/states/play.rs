use crate::loaders::gdlevel::GDColorChannel::{BaseColor, CopyColor};
use crate::loaders::gdlevel::{GDBaseColor, GDColorChannel};
use crate::states::loading::GlobalAssets;
use crate::{Cocos2dAtlas, GDSaveFile, GameState, ObjectMapping};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::utils::HashMap;

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_enter(GameState::Play).with_system(play_setup))
            .add_system_set(SystemSet::on_exit(GameState::Play).with_system(play_cleanup))
            .add_system_set(
                SystemSet::on_update(GameState::Play)
                    .with_system(move_camera)
                    .with_system(exit_play),
            );
    }
}

#[derive(Resource)]
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
    cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
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
        let mut atlas_mapping = 0;
        let mut texture_offset = Vec2::default();
        let mut texture_rotated = false;
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
                    Some((mapping, offset, rotated)) => {
                        atlas_handle = Some(packer_atlas.texture_atlas.clone());
                        atlas_mapping = *mapping;
                        texture_offset = *offset;
                        texture_rotated = *rotated;
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
                .spawn(SpriteSheetBundle {
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
                        scale: Vec3::new(
                            object.scale * if object.flip_x { -1. } else { 1. },
                            object.scale * if object.flip_y { -1. } else { 1. },
                            0.,
                        ),
                    },
                    sprite: TextureAtlasSprite {
                        index: atlas_mapping,
                        color: if level.start_object.colors.contains_key(&object.main_color) {
                            let (r, g, b, a) =
                                get_color(&level.start_object.colors, &object.main_color);
                            Color::rgba(r, g, b, a)
                        } else {
                            Color::WHITE
                        },
                        anchor: Anchor::Custom(texture_offset),
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

fn get_color(colors: &HashMap<u128, GDColorChannel>, index: &u128) -> (f32, f32, f32, f32) {
    match colors
        .get(index)
        .unwrap_or(&BaseColor(GDBaseColor::default()))
    {
        BaseColor(color) => (
            color.r as f32 / u8::MAX as f32,
            color.g as f32 / u8::MAX as f32,
            color.b as f32 / u8::MAX as f32,
            color.opacity,
        ),
        CopyColor(color) => {
            let (r, g, b, a) = get_color(colors, &color.copied_index);
            let (mut h, mut s, mut v) = rgb_to_hsv([r, g, b]);
            h += color.hsv.h;
            s *= color.hsv.s;
            v *= color.hsv.v;
            let [r, g, b] = hsv_to_rgb((h, s, v));
            (r, g, b, if color.copy_opacity { a } else { color.opacity })
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

fn exit_play(mut state: ResMut<State<GameState>>, keys: Res<Input<KeyCode>>) {
    if keys.pressed(KeyCode::Escape) {
        state.set(GameState::LevelSelect).unwrap()
    }
}

#[derive(Component)]
struct LevelObject;

fn play_cleanup(mut commands: Commands, query: Query<Entity, With<LevelObject>>) {
    query.for_each(|entity| commands.entity(entity).despawn_recursive());
}

#[inline(always)]
pub fn rgb_to_hsv(rgb: [f32; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb;
    let (max, min, diff, add) = {
        let (max, min, diff, add) = if r > g {
            (r, g, g - b, 0.0)
        } else {
            (g, r, b - r, 2.0)
        };
        if b > max {
            (b, min, r - g, 4.0)
        } else {
            (max, b.min(min), diff, add)
        }
    };

    let v = max;
    let h = if max == min {
        0.0
    } else {
        let mut h = 60.0 * (add + diff / (max - min));
        if h < 0.0 {
            h += 360.0;
        }
        h
    };
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };

    (h, s, v)
}

/// Convert hsv to rgb. Expects h [0, 360], s [0, 1], v [0, 1]
#[inline(always)]
pub fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> [f32; 3] {
    let c = s * v;
    let h = h / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if (0.0..=1.0).contains(&h) {
        (c, x, 0.0)
    } else if h <= 2.0 {
        (x, c, 0.0)
    } else if h <= 3.0 {
        (0.0, c, x)
    } else if h <= 4.0 {
        (0.0, x, c)
    } else if h <= 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r + m, g + m, b + m]
}
