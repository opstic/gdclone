use std::time::{Instant, SystemTime};

use bevy::prelude::*;
use discord_sdk::activity;
use discord_sdk::activity::ActivityBuilder;

use crate::discord::CurrentDiscordActivity;
use crate::level::{
    color::ColorChannels,
    object::Object,
    trigger::ExecutingTriggers,
    {Groups, Sections},
};
use crate::loader::{
    cocos2d_atlas::{Cocos2dAtlas, Cocos2dFrames},
    gdlevel::SaveFile,
};
use crate::state::{loading::GlobalAssets, GameState};

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(play_setup.in_schedule(OnEnter(GameState::Play)))
            .add_system(play_cleanup.in_schedule(OnExit(GameState::Play)))
            .add_systems(
                (move_camera, update_background_color, exit_play).in_set(OnUpdate(GameState::Play)),
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
    cocos2d_frames: Res<Cocos2dFrames>,
    cocos2d_atlases: Res<Assets<Cocos2dAtlas>>,
    mut sections: ResMut<Sections>,
    global_assets: Res<GlobalAssets>,
    save_file: Res<Assets<SaveFile>>,
    level_index: Res<LevelIndex>,
    mut discord_activity: ResMut<CurrentDiscordActivity>,
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

    discord_activity.0 = ActivityBuilder::default()
        .state(format!("Playing {}", level.name))
        .assets(activity::Assets::default().large("icon".to_owned(), Some("GDClone".to_owned())))
        .button(activity::Button {
            label: "Get GDClone".to_owned(),
            url: "https://github.com/opstic/gdclone/releases".to_owned(),
        })
        .start_timestamp(SystemTime::now())
        .into();

    info!("Loading {}", level.name);
    let total_start = Instant::now();
    let decompress_start = Instant::now();
    if let Some(Ok(decompressed_level)) = level.decompress_inner_level() {
        info!("Decompressing took {:?}", decompress_start.elapsed());
        let parse_start = Instant::now();
        if let Ok(parsed_level) = decompressed_level.parse() {
            info!("Parsing took {:?}", parse_start.elapsed());
            let spawn_start = Instant::now();
            parsed_level
                .spawn_level(
                    &mut commands,
                    &mut sections,
                    &cocos2d_frames,
                    &cocos2d_atlases,
                    false,
                )
                .unwrap();
            info!("Spawning took {:?}", spawn_start.elapsed());
            info!("Spawned {:?} objects", parsed_level.objects());
            info!("Total loading time is {:?}", total_start.elapsed());
        }
    }
}

#[derive(Component)]
pub(crate) struct Player(pub Vec2);

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
