use bevy::app::{
    App, First, Last, MainScheduleOrder, Plugin, PostUpdate, PreUpdate, RunFixedMainLoop, Update,
};
use bevy::audio::{AudioSink, AudioSinkPlayback};
use bevy::ecs::schedule::{ExecutorKind, ScheduleLabel};
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::math::{Vec2, Vec3Swizzles, Vec4Swizzles};
use bevy::prelude::{
    in_state, Camera, ClearColor, Color, Commands, Component, EventReader, GizmoPrimitive2d,
    Gizmos, GlobalTransform, IntoSystemConfigs, KeyCode, MouseButton, Mut, NextState, OnEnter,
    OnExit, OrthographicProjection, Query, Res, ResMut, Resource, Schedule, Transform, With,
};
use bevy_egui::EguiContexts;

use crate::level::color::{ColorChannelCalculated, GlobalColorChannels, ObjectColorCalculated};
use crate::level::player::Player;
use crate::level::section::GlobalSections;
use crate::level::transform::Transform2d;
use crate::level::LevelWorld;
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
pub(crate) struct SongPlayer;

#[derive(Resource)]
pub(crate) struct Options {
    show_options: bool,
    synchronize_cameras: bool,
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
            synchronize_cameras: true,
            display_simulated_camera: false,
            display_hitboxes: false,
            visible_sections_from_simulated: false,
            show_lines: false,
            hide_triggers: true,
            pause_player: false,
        }
    }
}

fn level_setup(mut options: ResMut<Options>) {
    options.pause_player = false;
}

fn render_option_gui(
    mut options: ResMut<Options>,
    mut contexts: EguiContexts,
    mut state: ResMut<NextState<GameState>>,
) {
    if !options.show_options {
        return;
    }

    egui::Window::new("Level Options").show(contexts.ctx_mut(), |ui| {
        ui.checkbox(&mut options.synchronize_cameras, "Synchronize cameras");
        ui.checkbox(&mut options.display_hitboxes, "Display hitboxes");
        ui.checkbox(&mut options.show_lines, "Display camera and player X");
        ui.checkbox(&mut options.hide_triggers, "Hide triggers");
        ui.checkbox(&mut options.pause_player, "Pause player");
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
) {
    if keys.just_pressed(KeyCode::Escape) {
        if !options.pause_player {
            options.show_options = true;
        }

        options.pause_player = !options.pause_player;
    }
    if keys.just_pressed(KeyCode::F7) {
        options.show_options = !options.show_options;
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
                if !options.synchronize_cameras {
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
    mut song_players: Query<&mut AudioSink, With<SongPlayer>>,
) {
    let LevelWorld::World(ref mut world) = *level_world else {
        panic!("World is supposed to be created");
    };

    world.run_schedule(First);
    world.run_schedule(PreUpdate);
    world.run_schedule(RunFixedMainLoop);

    if !options.pause_player {
        for sink in &mut song_players {
            sink.play();
        }
        world.run_schedule(Update);
    } else {
        for sink in &mut song_players {
            sink.pause();
        }
    }

    // Render player line
    let mut players = world.query::<(&Player, &Transform2d)>();

    if options.show_lines {
        for (player, transform) in players.iter(world) {
            let (player_line_start, player_line_end) = if player.vertical_is_x {
                (
                    Vec2::new(transform.translation.x - 500., transform.translation.y),
                    Vec2::new(transform.translation.x + 500., transform.translation.y),
                )
            } else {
                (
                    Vec2::new(transform.translation.x, transform.translation.y - 500.),
                    Vec2::new(transform.translation.x, transform.translation.y + 500.),
                )
            };
            gizmos.line_2d(player_line_start, player_line_end, Color::ORANGE_RED)
        }
    }

    let (camera_projection, mut camera_transform) = camera.single_mut();

    let (_, player_transform) = players.single(world);

    if options.synchronize_cameras {
        camera_transform.translation.x = player_transform.translation.x + 75.;
        if options.show_lines {
            gizmos.line_2d(
                Vec2::new(
                    camera_transform.translation.x,
                    camera_transform.translation.y - 500.,
                ),
                Vec2::new(
                    camera_transform.translation.x,
                    camera_transform.translation.y + 500.,
                ),
                Color::GREEN,
            );
            gizmos.line_2d(
                Vec2::new(
                    player_transform.translation.x,
                    camera_transform.translation.y - 500.,
                ),
                Vec2::new(
                    player_transform.translation.x,
                    camera_transform.translation.y + 500.,
                ),
                Color::ORANGE_RED,
            );
        }
    }

    let camera_min = camera_projection.area.min.x + camera_transform.translation.x;
    let camera_max = camera_projection.area.max.x + camera_transform.translation.x;

    let min_section = section_index_from_x(camera_min) as usize;
    let max_section = section_index_from_x(camera_max) as usize;

    let mut global_sections = world.resource_mut::<GlobalSections>();
    global_sections.visible = min_section.saturating_sub(2)..max_section.saturating_add(2);

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

fn level_cleanup(mut song_players: Query<&mut AudioSink, With<SongPlayer>>) {
    for sink in &mut song_players {
        sink.pause();
    }
}
