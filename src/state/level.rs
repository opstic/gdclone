use std::time::Duration;

use bevy::app::{
    App, First, Last, MainScheduleOrder, Plugin, PostUpdate, PreUpdate, RunFixedMainLoop, Update,
};
use bevy::asset::{Assets, Handle};
use bevy::ecs::schedule::{ExecutorKind, ScheduleLabel};
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::math::{Vec2, Vec3, Vec3Swizzles, Vec4Swizzles};
use bevy::prelude::{
    in_state, Camera, ClearColor, Color, Commands, Component, Entity, EventReader,
    GizmoPrimitive2d, Gizmos, GlobalTransform, IntoSystemConfigs, KeyCode, MouseButton, Mut,
    NextState, OnEnter, OnExit, OrthographicProjection, Query, Res, ResMut, Resource, Schedule,
    Transform, With,
};
use bevy::time::Time;
use bevy_egui::EguiContexts;
use bevy_kira_audio::{AudioInstance, AudioTween, PlaybackState};

use crate::level::color::{ColorChannelCalculated, GlobalColorChannels, ObjectColorCalculated};
use crate::level::player::Player;
use crate::level::section::GlobalSections;
use crate::level::transform::Transform2d;
use crate::level::trigger::GlobalTriggers;
use crate::level::{LevelWorld, SongOffset};
use crate::state::GameState;
use crate::utils::section_index_from_x;

pub(crate) struct LevelStatePlugin;

impl Plugin for LevelStatePlugin {
    fn build(&self, app: &mut App) {
        let mut level_schedule = Schedule::new(Level);
        level_schedule.set_executor_kind(ExecutorKind::SingleThreaded);

        app.add_schedule(level_schedule);

        app.world
            .resource_scope(|_, mut schedule_order: Mut<MainScheduleOrder>| {
                schedule_order.insert_after(Update, Level)
            });

        app.init_resource::<Options>()
            .add_systems(OnEnter(GameState::Level), level_setup)
            .add_systems(Level, update_level_world.run_if(in_state(GameState::Level)))
            .add_systems(
                Update,
                (update_controls, render_option_gui).run_if(in_state(GameState::Level)),
            )
            .add_systems(OnExit(GameState::Level), level_cleanup);
    }
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Level;

#[derive(Component)]
pub(crate) struct SongPlayer(pub(crate) Handle<AudioInstance>);

#[derive(Resource)]
pub(crate) struct Options {
    show_options: bool,
    lock_camera_to_player: bool,
    display_simulated_camera: bool,
    display_hitboxes: bool,
    visible_sections_from_simulated: bool,
    show_lines: bool,
    pub(crate) hide_triggers: bool,
    pause_player: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_options: false,
            lock_camera_to_player: true,
            display_simulated_camera: false,
            display_hitboxes: false,
            visible_sections_from_simulated: false,
            show_lines: false,
            hide_triggers: true,
            pause_player: false,
        }
    }
}

fn level_setup(
    mut options: ResMut<Options>,
    mut cameras: Query<(&mut Transform, &mut OrthographicProjection), With<Camera>>,
    mut level_world: ResMut<LevelWorld>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    song_players: Query<&SongPlayer>,
) {
    *options = Options::default();
    for (mut transform, mut projection) in &mut cameras {
        transform.translation = Vec3::ZERO;
        projection.scale = 1.;
    }
    let LevelWorld::World(ref mut world) = *level_world else {
        panic!("World is supposed to be created");
    };
    let mut players = world.query_filtered::<&Transform2d, With<Player>>();
    world.resource_scope(|world, song_offset: Mut<SongOffset>| {
        world.resource_scope(|world, global_triggers: Mut<GlobalTriggers>| {
            let transform = players.single(world);
            let mut time = global_triggers
                .speed_changes
                .time_for_pos(transform.translation.x);

            time += song_offset.0;

            if let Ok(song_player) = song_players.get_single() {
                if let Some(instance) = audio_instances.get_mut(&song_player.0) {
                    instance.seek_to(time as f64);
                    instance.resume(AudioTween::linear(Duration::ZERO));
                }
            }
        });
    })
}

fn render_option_gui(
    mut options: ResMut<Options>,
    mut contexts: EguiContexts,
    mut state: ResMut<NextState<GameState>>,
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
) {
    if !options.show_options {
        return;
    }

    egui::Window::new("Level Options").show(contexts.ctx_mut(), |ui| {
        ui.checkbox(&mut options.show_options, "Show options");
        ui.checkbox(
            &mut options.lock_camera_to_player,
            "Lock camera to player (U)",
        );
        ui.checkbox(&mut options.display_hitboxes, "Display hitboxes (H)");
        ui.checkbox(&mut options.show_lines, "Display camera and player X (L)");
        ui.checkbox(&mut options.hide_triggers, "Hide triggers (T)");
        ui.checkbox(&mut options.pause_player, "Pause player (Esc)");
        if ui.button("Reset zoom (R)").clicked() {
            for mut projection in &mut projections {
                projection.scale = 1.;
            }
        }
        ui.separator();
        if ui.button("Exit to menu").clicked() {
            state.set(GameState::Menu);
        }
    });
}

fn update_controls(
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
    mut transforms: Query<&mut Transform, With<Camera>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut options: ResMut<Options>,
    time: Res<Time>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if options.pause_player && !options.show_options {
            options.show_options = true;
        } else {
            options.pause_player = !options.pause_player;
            options.show_options = options.pause_player;
        }
    }
    if keys.just_pressed(KeyCode::KeyU) {
        options.lock_camera_to_player = !options.lock_camera_to_player;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        options.display_hitboxes = !options.display_hitboxes;
    }
    if keys.just_pressed(KeyCode::KeyL) {
        options.show_lines = !options.show_lines;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        options.hide_triggers = !options.hide_triggers;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        for mut projection in &mut projections {
            projection.scale = 1.;
        }
    }

    let multiplier = time.delta_seconds() * 20.;
    for mut transform in transforms.iter_mut() {
        if !options.lock_camera_to_player {
            if keys.pressed(KeyCode::ArrowRight) {
                transform.translation.x += 10.0 * multiplier;
            }
            if keys.pressed(KeyCode::ArrowLeft) {
                transform.translation.x -= 10.0 * multiplier;
            }
            if keys.pressed(KeyCode::KeyA) {
                transform.translation.x -= 20.0 * multiplier;
            }
            if keys.pressed(KeyCode::KeyD) {
                transform.translation.x += 20.0 * multiplier;
            }
        }
        if keys.pressed(KeyCode::ArrowUp) {
            transform.translation.y += 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            transform.translation.y -= 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::KeyW) {
            transform.translation.y += 20.0 * multiplier;
        }
        if keys.pressed(KeyCode::KeyS) {
            transform.translation.y -= 20.0 * multiplier;
        }
    }
    for mut projection in projections.iter_mut() {
        if keys.pressed(KeyCode::KeyQ) {
            projection.scale *= 1.01;
        }
        if keys.pressed(KeyCode::KeyE) {
            projection.scale *= 0.99;
        }
    }

    for mouse_wheel_event in mouse_wheel_events.read() {
        let dy = match mouse_wheel_event.unit {
            MouseScrollUnit::Line => mouse_wheel_event.y * 10.,
            MouseScrollUnit::Pixel => mouse_wheel_event.y,
        };

        for mut projection in projections.iter_mut() {
            projection.scale *= 1. + (-dy / 100.);
        }
    }

    let (camera, transform) = cameras.single();

    if mouse_button.pressed(MouseButton::Left) {
        for mouse_motion_event in mouse_motion_events.read() {
            let mut delta = camera
                .ndc_to_world(
                    transform,
                    (mouse_motion_event.delta * 2. / camera.logical_viewport_size().unwrap())
                        .extend(1.),
                )
                .unwrap()
                .xy()
                - transform.translation().xy();
            delta /= 1.75;
            for mut transform in transforms.iter_mut() {
                if !options.lock_camera_to_player {
                    transform.translation.x -= delta.x;
                }
                transform.translation.y += delta.y;
            }
        }
    }
}

fn update_level_world(
    mut commands: Commands,
    mut camera: Query<(&OrthographicProjection, &mut Transform)>,
    mut level_world: ResMut<LevelWorld>,
    options: ResMut<Options>,
    mut gizmos: Gizmos,
    song_players: Query<&SongPlayer>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    let LevelWorld::World(ref mut world) = *level_world else {
        panic!("World is supposed to be created");
    };

    world.run_schedule(First);
    world.run_schedule(PreUpdate);
    world.run_schedule(RunFixedMainLoop);

    if !options.pause_player {
        world.run_schedule(Update);
    }

    if let Ok(song_player) = song_players.get_single() {
        if let Some(instance) = audio_instances.get_mut(&song_player.0) {
            if options.pause_player {
                if let PlaybackState::Playing { .. } = instance.state() {
                    instance.pause(AudioTween::linear(Duration::ZERO));
                }
            } else if let PlaybackState::Paused { .. } = instance.state() {
                let mut players = world.query_filtered::<&Transform2d, With<Player>>();
                world.resource_scope(|world, song_offset: Mut<SongOffset>| {
                    world.resource_scope(|world, global_triggers: Mut<GlobalTriggers>| {
                        let transform = players.single(world);
                        let mut time = global_triggers
                            .speed_changes
                            .time_for_pos(transform.translation.x);

                        time += song_offset.0;

                        instance.seek_to(time as f64);
                    });
                });
                instance.resume(AudioTween::linear(Duration::ZERO));
            }
        }
    }

    // Render player line
    let mut players = world.query::<(&Player, &Transform2d)>();

    let (camera_projection, mut camera_transform) = camera.single_mut();

    if options.show_lines {
        for (_, transform) in players.iter(world) {
            gizmos.line_2d(
                Vec2::new(
                    transform.translation.x,
                    camera_transform.translation.y + camera_projection.area.min.y,
                ),
                Vec2::new(
                    transform.translation.x,
                    camera_transform.translation.y + camera_projection.area.max.y,
                ),
                Color::RED,
            );
        }
    }

    let (_, player_transform) = players.single(world);

    if options.lock_camera_to_player {
        camera_transform.translation.x = player_transform.translation.x + 75.;
        if options.show_lines {
            gizmos.line_2d(
                Vec2::new(
                    camera_transform.translation.x,
                    camera_transform.translation.y + camera_projection.area.min.y,
                ),
                Vec2::new(
                    camera_transform.translation.x,
                    camera_transform.translation.y + camera_projection.area.max.y,
                ),
                Color::GREEN,
            );
        }
    }

    let camera_min = camera_transform.translation.x + camera_projection.area.min.x;
    let camera_max = camera_transform.translation.x + camera_projection.area.max.x;

    let min_section = section_index_from_x(camera_min) as usize;
    let max_section = section_index_from_x(camera_max) as usize;

    let mut global_sections = world.resource_mut::<GlobalSections>();
    global_sections.visible = min_section.saturating_sub(2)..max_section.saturating_add(3);

    world.run_schedule(PostUpdate);
    world.run_schedule(Last);

    if options.display_hitboxes {
        world.resource_scope(|world, global_sections: Mut<GlobalSections>| {
            let mut query = world.query::<(
                &ObjectColorCalculated,
                &crate::level::collision::GlobalHitbox,
            )>();
            for section in &global_sections.sections[global_sections.visible.clone()] {
                for (object_calculated, hitbox) in query.iter_many(world, section) {
                    if !object_calculated.enabled {
                        continue;
                    }

                    gizmos.primitive_2d(*hitbox, Vec2::ZERO, 0., Color::BLUE);
                }
            }
        })
    }

    world.resource_scope(|world, global_color_channels: Mut<GlobalColorChannels>| {
        if let Some(entity) = global_color_channels.0.get(&1000) {
            let mut query = world.query::<&ColorChannelCalculated>();
            if let Ok(calculated) = query.get(world, *entity) {
                commands.insert_resource(ClearColor(Color::rgb_linear_from_array(
                    calculated.color.xyz(),
                )));
            }
        }
    });

    world.clear_trackers();
}

fn level_cleanup(
    mut commands: Commands,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    song_players: Query<(Entity, &SongPlayer)>,
) {
    for (entity, song_player) in &song_players {
        commands.entity(entity).despawn_recursive();

        let Some(instance) = audio_instances.get_mut(&song_player.0) else {
            continue;
        };

        instance.stop(AudioTween::linear(Duration::ZERO));
    }
}
