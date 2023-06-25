use crate::level::color::ColorChannels;
use std::time::Instant;

use crate::level::object::Object;
use crate::level::trigger::ExecutingTriggers;
use crate::level::{Groups, Sections};
use crate::loaders::gdlevel::SaveFile;
use crate::states::loading::GlobalAssets;

use crate::GameState;

use crate::loaders::cocos2d_atlas::{Cocos2dAtlas, Cocos2dFrames};
use bevy::prelude::*;

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(play_setup.in_schedule(OnEnter(GameState::Play)))
            .add_system(play_cleanup.in_schedule(OnExit(GameState::Play)))
            // .add_system_set_to_stage(
            //     CoreStage::PostUpdate,
            //     SystemSet::on_update(GameState::Play)
            //         .with_system(
            //             activate_xpos_trigger
            //                 .label(TriggerSystems::ActivateTriggers)
            //                 .after(TransformSystem::TransformPropagate),
            //         )
            //         .with_system(
            //             update_object_color
            //                 .after(VisibilitySystems::CheckVisibility)
            //                 .after(TriggerSystems::ActivateTriggers),
            //         ),
            // )
            .add_systems(
                (move_camera, update_background_color, exit_play).in_set(OnUpdate(GameState::Play)),
            );
        // .init_resource::<Groups>()
        // .init_resource::<ColorChannels>()
        // .register_type::<LevelObject>();
    }
}

#[derive(Resource)]
pub(crate) struct LevelIndex {
    pub(crate) index: usize,
}

// #[derive(Default, Resource)]
// pub(crate) struct Groups {
//     pub(crate) groups: HashMap<u64, Vec<Entity>>,
// }

fn play_setup(
    mut camera_transforms: Query<&mut Transform, With<Camera>>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
    mut commands: Commands,
    cocos2d_frames: Res<Cocos2dFrames>,
    cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
    mut sections: ResMut<Sections>,
    global_assets: Res<GlobalAssets>,
    save_file: Res<Assets<SaveFile>>,
    level_index: Res<LevelIndex>,
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
    info!("Loading {}", level.name);
    let total_time = Instant::now();
    let decompress_time = Instant::now();
    if let Some(Ok(decompressed_level)) = level.decompress_inner_level() {
        info!("Decompressing took {:?}", decompress_time.elapsed());
        let parse_time = Instant::now();
        if let Ok(parsed_level) = decompressed_level.parse() {
            info!("Parsing took {:?}", parse_time.elapsed());
            let spawn_time = Instant::now();
            parsed_level
                .spawn_level(
                    &mut commands,
                    &mut sections,
                    &cocos2d_frames,
                    &cocos2d_atlases,
                    false,
                )
                .unwrap();
            info!("Spawned {:?} objects", parsed_level.objects());
            info!("Spawning took {:?}", spawn_time.elapsed());
            info!("Total loading time is {:?}", total_time.elapsed());
        }
    }
}

// #[derive(Component)]
// pub(crate) struct ObjectColor(
//     pub(crate) u64,
//     pub(crate) GDHSV,
//     pub(crate) f32,
//     pub(crate) f32,
// );

#[derive(Component)]
pub(crate) struct Player(pub Vec2);

// fn get_texture(
//     mapping: &HashMap<u64, String>,
//     atlases: &Vec<&Cocos2dAtlas>,
//     id: &u16,
// ) -> Option<(Handle<TextureAtlas>, usize, Vec2, bool)> {
//     let texture_name = mapping.get(&(*id as u64));
//     if let Some(name) = texture_name {
//         let mut atlas_handle: Handle<TextureAtlas> = Default::default();
//         let mut atlas_mapping = 0;
//         let mut texture_offset = Vec2::default();
//         let mut texture_rotated = false;
//         for atlas in atlases {
//             match atlas.index.get(name) {
//                 Some((mapping, offset, rotated)) => {
//                     atlas_handle = atlas.texture_atlas.clone();
//                     atlas_mapping = *mapping;
//                     texture_offset = *offset;
//                     texture_rotated = *rotated;
//                     break;
//                 }
//                 None => continue,
//             }
//         }
//         Some((atlas_handle, atlas_mapping, texture_offset, texture_rotated))
//     } else {
//         None
//     }
// }
//
// fn get_color(colors: &HashMap<u64, GDColorChannel>, index: &u64) -> (Color, bool) {
//     match colors
//         .get(index)
//         .unwrap_or(&BaseColor(GDBaseColor::default()))
//     {
//         BaseColor(color) => (
//             Color::rgba(
//                 color.r as f32 / u8::MAX as f32,
//                 color.g as f32 / u8::MAX as f32,
//                 color.b as f32 / u8::MAX as f32,
//                 color.opacity,
//             ),
//             color.blending,
//         ),
//         CopyColor(color) => {
//             let (original_color, _) = get_color(colors, &color.copied_index);
//             let mut transformed_color = apply_hsv(original_color, &color.hsv);
//             if !color.copy_opacity {
//                 transformed_color.set_a(color.opacity);
//             }
//             (transformed_color, color.blending)
//         }
//     }
// }

// fn update_object_color(
//     mut commands: Commands,
//     mut objects: Query<
//         (
//             Entity,
//             &Visibility,
//             &ComputedVisibility,
//             &ObjectColor,
//             Option<&mut TextureAtlasSprite>,
//             Option<&mut Text>,
//         ),
//         With<LevelObject>,
//     >,
//     mut cameras: Query<&mut Camera2d>,
//     color_channel: Res<ColorChannels>,
// ) {
//     if color_channel.colors.contains_key(&1000) {
//         let (mut color, _) = get_color(&color_channel.colors, &1000);
//         for mut camera in cameras.iter_mut() {
//             camera.clear_color = ClearColorConfig::Custom(*color.set_a(1.));
//         }
//     }
//     let command_mutex = Arc::new(Mutex::new(commands));
//     objects.par_for_each_mut(
//         512,
//         |(entity, visibility, computed_visibility, object_color, mut sprite, mut text)| {
//             if !visibility.is_visible || !computed_visibility.is_visible() {
//                 return;
//             }
//             let ObjectColor(channel, hsv, opacity, _) = object_color;
//             let (color, blending) = get_color(&color_channel.colors, &channel);
//             let mut final_color = apply_hsv(color, &hsv);
//             let col_opacity = final_color.a();
//             if blending {
//                 let mut opacity = col_opacity * opacity;
//                 // TODO: additive blending is weird for some reason...
//                 // Fix this when wgpu solves this problem
//                 opacity = f32::powf(opacity, 4.475);
//                 final_color = *final_color.set_a(opacity);
//             } else {
//                 final_color = *final_color.set_a(col_opacity * opacity);
//             }
//             if let Some(mut sprite) = sprite {
//                 sprite.color = final_color;
//             }
//             if let Some(mut text) = text {
//                 for section in &mut text.sections {
//                     section.style.color = final_color;
//                 }
//             }
//             if let Ok(mut commands) = command_mutex.lock() {
//                 if blending {
//                     commands.entity(entity).insert(BlendingSprite);
//                 } else {
//                     commands.entity(entity).remove::<BlendingSprite>();
//                 }
//             }
//         },
//     )
// }
//
// fn activate_xpos_trigger(
//     mut commands: Commands,
//     triggers: Query<
//         (Entity, &Transform, &Visibility),
//         (
//             With<XPosActivate>,
//             Without<TriggerInProgress>,
//             Without<TriggerActivated>,
//         ),
//     >,
//     camera_transforms: Query<&Transform, (With<Camera2d>, Without<LevelObject>)>,
// ) {
//     let player_x = if let Ok(transform) = camera_transforms.get_single() {
//         transform.translation.x
//     } else {
//         return;
//     };
//     let command_mutex = Arc::new(Mutex::new(commands));
//     triggers.par_for_each(512, |(entity, transform, visibility)| {
//         if transform.translation.x > player_x || !visibility.is_visible {
//             return;
//         }
//         if let Ok(mut commands) = command_mutex.lock() {
//             commands.entity(entity).insert(TriggerInProgress);
//         }
//     });
// }

fn move_camera(
    mut camera_transforms: Query<&mut Transform, With<Camera>>,
    keys: Res<Input<KeyCode>>,
    time: Res<Time>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
) {
    let delta = time.delta_seconds();
    let multiplier = if keys.pressed(KeyCode::LShift) {
        40. * delta
    } else {
        20. * delta
    };
    for mut transform in camera_transforms.iter_mut() {
        if keys.pressed(KeyCode::Right) {
            transform.translation.x += 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::Left) {
            transform.translation.x -= 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::Up) {
            transform.translation.y += 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::Down) {
            transform.translation.y -= 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::A) {
            transform.translation.x -= 20.0 * multiplier;
        }
        if keys.pressed(KeyCode::D) {
            transform.translation.x += 20.0 * multiplier;
        }
        if keys.pressed(KeyCode::W) {
            transform.translation.y += 20.0 * multiplier;
        }
        if keys.pressed(KeyCode::S) {
            transform.translation.y -= 20.0 * multiplier;
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

fn update_background_color(
    color_channels: Res<ColorChannels>,
    mut clear_color: ResMut<ClearColor>,
) {
    let (color, _) = color_channels.get_color(&1000);
    clear_color.0 = color;
}

fn exit_play(
    mut next_state: ResMut<NextState<GameState>>,
    keys: Res<Input<KeyCode>>,
    mut executing_triggers: ResMut<ExecutingTriggers>,
) {
    if keys.pressed(KeyCode::Escape) {
        executing_triggers.0.clear();
        next_state.set(GameState::LevelSelect);
    }
}

// #[derive(Component, Default, Reflect)]
// #[reflect(Component)]
// pub(crate) struct LevelObject(GDLevelObject);

fn play_cleanup(
    mut commands: Commands,
    query: Query<Entity, (With<Object>, Without<Parent>)>,
    mut color_channels: ResMut<ColorChannels>,
    mut groups: ResMut<Groups>,
    mut clear_color: ResMut<ClearColor>,
    mut sections: ResMut<Sections>,
) {
    color_channels.0.clear();
    groups.0.clear();
    sections.0.clear();
    clear_color.0 = Color::GRAY;
    query.for_each(|entity| commands.entity(entity).despawn_recursive());
}
