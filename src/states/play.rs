use crate::level::easing::Easing;
use crate::level::trigger::alpha::AlphaTrigger;
use crate::level::trigger::color::ColorTrigger;
use crate::level::trigger::r#move::MoveTrigger;
use crate::level::trigger::rotate::RotateTrigger;
use crate::level::trigger::toggle::ToggleTrigger;
use crate::level::trigger::{
    MultiActivate, Trigger, TriggerActivated, TriggerCompleted, TriggerDuration, TriggerFunction,
    TriggerInProgress, TriggerSystems, XPosActivate,
};
use crate::loaders::gdlevel::GDColorChannel::{BaseColor, CopyColor};
use crate::loaders::gdlevel::{GDBaseColor, GDColorChannel, GDLevelObject, GDHSV};
use crate::render::sprite::BlendingSprite;
use crate::states::loading::GlobalAssets;
use crate::utils::u8_to_bool;
use crate::{Cocos2dAtlas, GDSaveFile, GameState, Mapping};
use bevy::core_pipeline::clear_color::ClearColorConfig;
use bevy::prelude::system_adapter::unwrap;
use bevy::prelude::*;
use bevy::render::view::VisibilitySystems;
use bevy::sprite::Anchor;
use bevy::transform::TransformSystem;
use bevy::utils::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_state_to_stage(CoreStage::PostUpdate, GameState::Play)
            .add_system_set(SystemSet::on_enter(GameState::Play).with_system(play_setup))
            .add_system_set(SystemSet::on_exit(GameState::Play).with_system(play_cleanup))
            .add_system_set_to_stage(
                CoreStage::PostUpdate,
                SystemSet::on_update(GameState::Play)
                    .with_system(
                        activate_xpos_trigger
                            .label(TriggerSystems::ActivateTriggers)
                            .after(TransformSystem::TransformPropagate),
                    )
                    .with_system(
                        update_object_color
                            .after(VisibilitySystems::CheckVisibility)
                            .after(TriggerSystems::ActivateTriggers),
                    ),
            )
            .add_system_set(
                SystemSet::on_update(GameState::Play)
                    .with_system(move_camera)
                    .with_system(exit_play),
            )
            .init_resource::<Groups>()
            .init_resource::<ColorChannels>()
            .register_type::<LevelObject>();
    }
}

#[derive(Resource)]
pub(crate) struct LevelIndex {
    pub(crate) index: usize,
}

#[derive(Default, Resource)]
pub(crate) struct Groups {
    pub(crate) groups: HashMap<u64, Vec<Entity>>,
}

#[derive(Default, Resource)]
pub(crate) struct ColorChannels {
    pub(crate) colors: HashMap<u64, GDColorChannel>,
}

fn play_setup(
    mut camera_transforms: Query<&mut Transform, With<Camera>>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
    mut cameras: Query<&mut Camera2d>,
    mut commands: Commands,
    mut groups_res: ResMut<Groups>,
    mut colors_res: ResMut<ColorChannels>,
    global_assets: Res<GlobalAssets>,
    save_file: Res<Assets<GDSaveFile>>,
    level_index: Res<LevelIndex>,
    mapping: Res<Assets<Mapping>>,
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

    let atlases = vec![
        cocos2d_atlases.get(&global_assets.atlas1).unwrap(),
        cocos2d_atlases.get(&global_assets.atlas2).unwrap(),
        cocos2d_atlases.get(&global_assets.atlas3).unwrap(),
        cocos2d_atlases.get(&global_assets.atlas4).unwrap(),
        cocos2d_atlases.get(&global_assets.atlas5).unwrap(),
    ];
    let mut groups: HashMap<u64, Vec<Entity>> = HashMap::new();
    for object in &level.inner_level {
        let (color, blending) = {
            let (color, blending) = get_color(&level.start_object.colors, &object.main_color);
            (apply_hsv(color, &object.main_hsv), blending)
        };
        let mut entity = if let Some((handle, mapping, offset, rotated)) = get_texture(
            &mapping.get(&global_assets.texture_mapping).unwrap().mapping,
            &atlases,
            &object.id,
        ) {
            let mut object_data = object.clone();
            if rotated {
                std::mem::swap(&mut object_data.flip_x, &mut object_data.flip_y);
            }
            commands.spawn(SpriteSheetBundle {
                transform: Transform {
                    translation: Vec3::from((
                        object_data.x,
                        object_data.y,
                        (object_data.z_layer + 3 - if blending { 1 } else { 0 }) as f32 * 100.
                            + (object_data.z_order + 999) as f32 * 100. / (999. + 10000.)
                            - 0.099,
                    )),
                    rotation: Quat::from_rotation_z(
                        -(object_data.rot + if rotated { -90. } else { 0. }).to_radians(),
                    ),
                    scale: Vec3::new(
                        object_data.scale * if object_data.flip_x { -1. } else { 1. },
                        object_data.scale * if object_data.flip_y { -1. } else { 1. },
                        0.,
                    ),
                },
                sprite: TextureAtlasSprite {
                    index: mapping,
                    color: color,
                    anchor: Anchor::Custom(offset),
                    ..Default::default()
                },
                texture_atlas: handle,
                ..default()
            })
        } else {
            match object.id {
                914 => {
                    let object_string = String::from_utf8_lossy(
                        &base64::decode_engine(object.other.get("31").unwrap(), &BASE64_URL_SAFE)
                            .unwrap(),
                    )
                    .to_string();
                    commands.spawn(Text2dBundle {
                        text: Text::from_section(
                            object_string,
                            TextStyle {
                                font: global_assets.font.clone(),
                                font_size: 180.0,
                                color: color,
                            },
                        )
                        .with_alignment(TextAlignment::CENTER),
                        transform: Transform {
                            translation: Vec3::from((
                                object.x,
                                object.y,
                                (object.z_layer + 3 - if blending { 1 } else { 0 }) as f32 * 100.
                                    + (object.z_order + 999) as f32 * 100. / (999. + 10000.)
                                    - 0.099,
                            )),
                            rotation: Quat::from_rotation_z(-object.rot.to_radians()),
                            scale: Vec3::new(
                                object.scale * if object.flip_x { -1. } else { 1. },
                                object.scale * if object.flip_y { -1. } else { 1. },
                                0.,
                            ),
                        },
                        ..default()
                    })
                }
                _ => continue,
            }
        };
        entity.insert(LevelObject(object.clone()));
        if blending {
            entity.insert(BlendingSprite);
        }
        match object.id {
            901 => {
                entity
                    .insert(XPosActivate)
                    .insert(Trigger(Box::new(MoveTrigger {
                        duration: TriggerDuration::new(Duration::from_secs_f64(
                            object.other.get("10").unwrap().parse().unwrap(),
                            // 0.,
                        )),
                        amount: 0.,
                        previous_amount: 0.,
                        easing: Easing::from_id(
                            object.other.get("30").unwrap().parse().unwrap(),
                            match object.other.get("85") {
                                Some(rate) => Some(rate.parse().unwrap()),
                                None => None,
                            },
                        ),
                        target_group: object.other.get("51").unwrap().parse().unwrap(),
                        x_offset: object.other.get("28").unwrap().parse().unwrap(),
                        y_offset: object.other.get("29").unwrap().parse().unwrap(),
                        lock_x: match object.other.get("58") {
                            Some(val) => u8_to_bool(&val.clone().into_bytes()),
                            None => false,
                        },
                        lock_y: match object.other.get("59") {
                            Some(val) => u8_to_bool(&val.clone().into_bytes()),
                            None => false,
                        },
                        player_x: 0.,
                        player_y: 0.,
                        player_previous_x: 0.,
                        player_previous_y: 0.,
                    })));
            }
            1007 => {
                entity
                    .insert(XPosActivate)
                    .insert(Trigger(Box::new(AlphaTrigger {
                        duration: TriggerDuration::new(
                            Duration::try_from_secs_f64(
                                object.other.get("10").unwrap().parse().unwrap(),
                            )
                            .unwrap_or_default(),
                        ),
                        target_group: object.other.get("51").unwrap().parse().unwrap(),
                        target_opacity: object.other.get("35").unwrap().parse().unwrap(),
                    })));
            }
            1346 => {
                entity
                    .insert(XPosActivate)
                    .insert(Trigger(Box::new(RotateTrigger {
                        duration: TriggerDuration::new(
                            Duration::try_from_secs_f64(
                                object.other.get("10").unwrap().parse().unwrap(),
                            )
                            .unwrap_or_default(),
                        ),
                        easing: Easing::from_id(
                            object.other.get("30").unwrap().parse().unwrap(),
                            match object.other.get("85") {
                                Some(rate) => Some(rate.parse().unwrap()),
                                None => None,
                            },
                        ),
                        target_group: object.other.get("51").unwrap().parse().unwrap(),
                        center_group: match object.other.get("71") {
                            Some(val) => val.parse().unwrap(),
                            None => 0,
                        },
                        degrees: object.other.get("68").unwrap().parse().unwrap(),
                        times360: object.other.get("69").unwrap().parse().unwrap(),
                        amount: 0.0,
                        previous_amount: 0.0,
                        center_translation: Default::default(),
                    })));
            }
            1049 => {
                entity
                    .insert(XPosActivate)
                    .insert(Trigger(Box::new(ToggleTrigger {
                        target_group: object.other.get("51").unwrap().parse().unwrap(),
                        activate: match object.other.get("56") {
                            Some(val) => u8_to_bool(&val.clone().into_bytes()),
                            None => false,
                        },
                    })));
            }
            899 => {
                entity
                    .insert(XPosActivate)
                    .insert(Trigger(Box::new(ColorTrigger {
                        duration: TriggerDuration::new(
                            Duration::try_from_secs_f64(
                                object.other.get("10").unwrap().parse().unwrap(),
                            )
                            .unwrap_or_default(),
                        ),
                        target_channel: match object.other.get("23") {
                            Some(val) => val.parse().unwrap(),
                            None => 1,
                        },
                        target_r: object.other.get("7").unwrap().parse().unwrap(),
                        target_g: object.other.get("8").unwrap().parse().unwrap(),
                        target_b: object.other.get("9").unwrap().parse().unwrap(),
                        target_opacity: object.other.get("35").unwrap().parse().unwrap(),
                        target_blending: match object.other.get("17") {
                            Some(val) => u8_to_bool(&val.clone().into_bytes()),
                            None => false,
                        },
                    })));
            }
            _ => {}
        }
        if object.id != 901 || object.id != 1007 || object.id != 1346 {
            for group in &object.groups {
                let entry = groups.entry(*group);
                entry.or_default().push(entity.id());
            }
        }
        entity.insert(ObjectColor(
            object.main_color,
            object.main_hsv.clone(),
            1.,
            1.,
        ));
    }
    if level.start_object.colors.contains_key(&1000) {
        let (mut color, _) = get_color(&level.start_object.colors, &1000);
        for mut camera in cameras.iter_mut() {
            camera.clear_color = ClearColorConfig::Custom(*color.set_a(1.));
        }
    }
    groups_res.groups = groups;
    colors_res.colors = level.start_object.colors.clone();
}

#[derive(Component)]
pub(crate) struct ObjectColor(
    pub(crate) u64,
    pub(crate) GDHSV,
    pub(crate) f32,
    pub(crate) f32,
);

#[derive(Component)]
pub(crate) struct Player;

fn get_texture(
    mapping: &HashMap<u64, String>,
    atlases: &Vec<&Cocos2dAtlas>,
    id: &u16,
) -> Option<(Handle<TextureAtlas>, usize, Vec2, bool)> {
    let texture_name = mapping.get(&(*id as u64));
    if let Some(name) = texture_name {
        let mut atlas_handle: Handle<TextureAtlas> = Default::default();
        let mut atlas_mapping = 0;
        let mut texture_offset = Vec2::default();
        let mut texture_rotated = false;
        for atlas in atlases {
            match atlas.index.get(name) {
                Some((mapping, offset, rotated)) => {
                    atlas_handle = atlas.texture_atlas.clone();
                    atlas_mapping = *mapping;
                    texture_offset = *offset;
                    texture_rotated = *rotated;
                    break;
                }
                None => continue,
            }
        }
        Some((atlas_handle, atlas_mapping, texture_offset, texture_rotated))
    } else {
        None
    }
}

fn get_color(colors: &HashMap<u64, GDColorChannel>, index: &u64) -> (Color, bool) {
    match colors
        .get(index)
        .unwrap_or(&BaseColor(GDBaseColor::default()))
    {
        BaseColor(color) => (
            Color::rgba(
                color.r as f32 / u8::MAX as f32,
                color.g as f32 / u8::MAX as f32,
                color.b as f32 / u8::MAX as f32,
                color.opacity,
            ),
            color.blending,
        ),
        CopyColor(color) => {
            let (original_color, _) = get_color(colors, &color.copied_index);
            let mut transformed_color = apply_hsv(original_color, &color.hsv);
            if !color.copy_opacity {
                transformed_color.set_a(color.opacity);
            }
            (transformed_color, color.blending)
        }
    }
}

fn apply_hsv(color: Color, hsv: &GDHSV) -> Color {
    let (h, s, v) = rgb_to_hsv([color.r(), color.g(), color.b()]);
    let [r, g, b] = hsv_to_rgb((h + hsv.h, s * hsv.s, v * hsv.v));
    Color::rgba(r, g, b, color.a())
}

fn update_object_color(
    mut commands: Commands,
    mut objects: Query<
        (
            Entity,
            &Visibility,
            &ComputedVisibility,
            &ObjectColor,
            Option<&mut TextureAtlasSprite>,
            Option<&mut Text>,
        ),
        With<LevelObject>,
    >,
    mut cameras: Query<&mut Camera2d>,
    color_channel: Res<ColorChannels>,
) {
    if color_channel.colors.contains_key(&1000) {
        let (mut color, _) = get_color(&color_channel.colors, &1000);
        for mut camera in cameras.iter_mut() {
            camera.clear_color = ClearColorConfig::Custom(*color.set_a(1.));
        }
    }
    let command_mutex = Arc::new(Mutex::new(commands));
    objects.par_for_each_mut(
        512,
        |(entity, visibility, computed_visibility, object_color, mut sprite, mut text)| {
            if !visibility.is_visible || !computed_visibility.is_visible() {
                return;
            }
            let ObjectColor(channel, hsv, opacity, _) = object_color;
            let (color, blending) = get_color(&color_channel.colors, &channel);
            let mut final_color = apply_hsv(color, &hsv);
            let col_opacity = final_color.a();
            if blending {
                let mut opacity = col_opacity * opacity;
                // TODO: additive blending is weird for some reason...
                // Fix this when wgpu solves this problem
                opacity = f32::powf(opacity, 4.475);
                final_color = *final_color.set_a(opacity);
            } else {
                final_color = *final_color.set_a(col_opacity * opacity);
            }
            if let Some(mut sprite) = sprite {
                sprite.color = final_color;
            }
            if let Some(mut text) = text {
                for section in &mut text.sections {
                    section.style.color = final_color;
                }
            }
            if let Ok(mut commands) = command_mutex.lock() {
                if blending {
                    commands.entity(entity).insert(BlendingSprite);
                } else {
                    commands.entity(entity).remove::<BlendingSprite>();
                }
            }
        },
    )
}

fn activate_xpos_trigger(
    mut commands: Commands,
    triggers: Query<
        (Entity, &Transform, &Visibility),
        (
            With<XPosActivate>,
            Without<TriggerInProgress>,
            Without<TriggerActivated>,
        ),
    >,
    camera_transforms: Query<&Transform, (With<Camera2d>, Without<LevelObject>)>,
) {
    let player_x = if let Ok(transform) = camera_transforms.get_single() {
        transform.translation.x
    } else {
        return;
    };
    let command_mutex = Arc::new(Mutex::new(commands));
    triggers.par_for_each(512, |(entity, transform, visibility)| {
        if transform.translation.x > player_x || !visibility.is_visible {
            return;
        }
        if let Ok(mut commands) = command_mutex.lock() {
            commands.entity(entity).insert(TriggerInProgress);
        }
    });
}

fn move_camera(
    mut camera_transforms: Query<&mut Transform, With<Camera>>,
    keys: Res<Input<KeyCode>>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
) {
    let multiplier = if keys.pressed(KeyCode::LShift) {
        2.
    } else {
        1.
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

fn exit_play(mut state: ResMut<State<GameState>>, keys: Res<Input<KeyCode>>) {
    if keys.pressed(KeyCode::Escape) {
        state.set(GameState::LevelSelect).unwrap()
    }
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub(crate) struct LevelObject(GDLevelObject);

fn play_cleanup(
    mut commands: Commands,
    query: Query<Entity, With<LevelObject>>,
    mut groups: ResMut<Groups>,
) {
    groups.groups.clear();
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

const BASE64_URL_SAFE: base64::engine::fast_portable::FastPortable =
    base64::engine::fast_portable::FastPortable::from(
        &base64::alphabet::URL_SAFE,
        base64::engine::fast_portable::PAD,
    );
