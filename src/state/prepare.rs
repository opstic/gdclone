use bevy::app::{App, Plugin, Update};
use bevy::asset::io::AssetSourceId;
use bevy::asset::{AssetPath, AssetServer, Handle, LoadState};
use bevy::hierarchy::{BuildChildren, DespawnRecursiveExt};
use bevy::log::{error, info};
use bevy::prelude::{
    default, in_state, AlignItems, ButtonInput, Color, Commands, Component, Entity,
    IntoSystemConfigs, JustifyContent, KeyCode, Local, NextState, NodeBundle, OnEnter, OnExit,
    Query, Res, ResMut, Resource, Style, Text, TextBundle, TextSection, TextStyle, Val, With,
    World,
};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::ui::FlexDirection;
use bevy_kira_audio::{Audio, AudioControl, AudioSource};
use futures_lite::future;
use instant::Instant;

use crate::api::{DefaultApi, ServerApi};
use crate::asset::cocos2d_atlas::Cocos2dFrames;
use crate::level::{LevelData, LevelInfo, LevelWorld, SongInfo};
use crate::state::level::SongPlayer;
use crate::state::menu::LevelBrowserState;
use crate::state::GameState;

pub(crate) struct PrepareStatePlugin;

impl Plugin for PrepareStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Prepare), prepare_setup)
            .add_systems(
                Update,
                (wait_for_creation, update_controls).run_if(in_state(GameState::Prepare)),
            )
            .add_systems(OnExit(GameState::Prepare), prepare_cleanup);
    }
}

#[derive(Component)]
struct PrepareText;

#[derive(Resource)]
pub(crate) struct LevelToDownload(pub(crate) LevelInfo);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct LevelDownloadTask(Task<Result<LevelData, anyhow::Error>>);

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
struct LevelDownloadTask(crossbeam_channel::Receiver<Result<LevelData, anyhow::Error>>);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct AudioDownloadTask(Task<Result<(u64, AudioSource), anyhow::Error>>);

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
struct AudioDownloadTask(crossbeam_channel::Receiver<Result<(u64, AudioSource), anyhow::Error>>);

#[derive(Resource)]
struct LocalSongHandle(SongInfo, Handle<AudioSource>);

fn prepare_setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    level_to_download: Res<LevelToDownload>,
    browser_state: Res<LevelBrowserState>,
    audio: Res<Audio>,
) {
    commands
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                ..default()
            },
            ..default()
        })
        .insert(PrepareText)
        .with_children(|parent| {
            parent
                .spawn(TextBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        width: Val::Percent(80.),
                        height: Val::Auto,
                        ..default()
                    },
                    text: Text {
                        sections: vec![
                            TextSection {
                                value: "".to_string(),
                                style: TextStyle {
                                    font_size: 40.,
                                    color: Color::WHITE,
                                    ..default()
                                },
                            },
                            TextSection {
                                value: "".to_string(),
                                style: TextStyle {
                                    font_size: 40.,
                                    color: Color::WHITE,
                                    ..default()
                                },
                            },
                        ],
                        ..default()
                    },
                    ..default()
                })
                .insert(PrepareText);
        });

    let level_info = level_to_download.0.clone();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let async_pool = AsyncComputeTaskPool::get();
        commands.insert_resource(LevelDownloadTask(async_pool.spawn(async move {
            info!("Downloading {}, ID: {}", level_info.name, level_info.id);
            let start = Instant::now();
            let api = DefaultApi::default();
            let level_data = api.get_level_data(level_info.id).await;
            info!("Download took {:?}", start.elapsed());
            level_data
        })));
    }

    #[cfg(target_arch = "wasm32")]
    {
        let level_info = level_to_download.0.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        wasm_bindgen_futures::spawn_local(async move {
            info!("Downloading {}, ID: {}", level_info.name, level_info.id);
            let start = Instant::now();
            let api = DefaultApi::default();
            let level_data = api.get_level_data(level_info.id).await;
            info!("Download took {:?}", start.elapsed());
            let _ = tx.send(level_data);
        });
        commands.insert_resource(LevelDownloadTask(rx));
    }

    if !browser_state.use_song {
        return;
    }

    if let Some(audio_handle) = browser_state.stored_songs.get(&level_info.song_id) {
        let instance_handle = audio.play(audio_handle.clone()).paused().handle();
        commands.spawn(SongPlayer(instance_handle));
        return;
    }

    let Some(song_info) = browser_state.song_infos.get(&level_info.song_id).cloned() else {
        return;
    };

    let data_source = AssetSourceId::from("data");
    let local_song: Handle<AudioSource> =
        server.load(AssetPath::from(song_info.id.to_string() + ".mp3").with_source(data_source));

    commands.insert_resource(LocalSongHandle(song_info, local_song));
}

fn update_controls(input: Res<ButtonInput<KeyCode>>, mut state: ResMut<NextState<GameState>>) {
    if input.just_pressed(KeyCode::Escape) {
        state.set(GameState::Menu);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn wait_for_creation(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut level_download_task: Option<ResMut<LevelDownloadTask>>,
    mut audio_download_task: Option<ResMut<AudioDownloadTask>>,
    mut level_world: Option<ResMut<LevelWorld>>,
    local_song_handle: Option<Res<LocalSongHandle>>,
    mut state: ResMut<NextState<GameState>>,
    cocos2d_frames: Res<Cocos2dFrames>,
    mut text_query: Query<&mut Text, With<PrepareText>>,
    mut browser_state: ResMut<LevelBrowserState>,
    audio: Res<Audio>,
) {
    if let Some(ref mut level_download_task) = level_download_task {
        if let Some(downloaded) = future::block_on(future::poll_once(&mut level_download_task.0)) {
            let async_pool = AsyncComputeTaskPool::get();

            let level_data = match downloaded {
                Ok(level_data) => level_data,
                Err(err) => {
                    error!("Level download failed. {}", err);
                    state.set(GameState::Menu);
                    return;
                }
            };

            info!("Starting world creation...");

            let cocos2d_frames = cocos2d_frames.clone();
            let low_detail = browser_state.low_detail;
            commands.insert_resource(LevelWorld::Pending(async_pool.spawn(async move {
                let start_all = Instant::now();
                let mut start = Instant::now();
                let decompressed = level_data.decompress_inner_level().unwrap()?;
                info!("Decompressing took {:?}", start.elapsed());
                start = Instant::now();
                let parsed = decompressed.parse()?;
                info!("Parsing took {:?}", start.elapsed());
                let world = parsed.create_world(&cocos2d_frames, low_detail);
                info!("Total time: {:?}", start_all.elapsed());

                Ok(world)
            })));
            commands.remove_resource::<LevelDownloadTask>();
        } else {
            text_query.single_mut().sections[0].value = "Downloading level\n".to_string();
        };
    }

    if let Some(ref local_song_handle) = local_song_handle {
        match asset_server.load_state(local_song_handle.1.clone()) {
            LoadState::Loaded => {
                text_query.single_mut().sections[1].value = "".to_string();
                browser_state
                    .stored_songs
                    .insert(local_song_handle.0.id, local_song_handle.1.clone());
                let instance_handle = audio.play(local_song_handle.1.clone()).paused().handle();
                commands.spawn(SongPlayer(instance_handle));
                commands.remove_resource::<LocalSongHandle>()
            }
            LoadState::Failed => {
                text_query.single_mut().sections[1].value = "".to_string();
                let async_pool = AsyncComputeTaskPool::get();
                let song_info = local_song_handle.0.clone();
                commands.insert_resource(AudioDownloadTask(async_pool.spawn(async move {
                    info!("Downloading song {}, ID: {}", song_info.name, song_info.id);
                    let start = Instant::now();
                    let api = DefaultApi::default();
                    let audio_source = api.get_song(song_info.clone()).await?;
                    info!("Song download took {:?}", start.elapsed());
                    Ok((song_info.id, audio_source))
                })));
                commands.remove_resource::<LocalSongHandle>()
            }
            _ => {
                text_query.single_mut().sections[1].value = "Loading local song".to_string();
            }
        }
    }

    if let Some(ref mut audio_download_task) = audio_download_task {
        if let Some(downloaded) = future::block_on(future::poll_once(&mut audio_download_task.0)) {
            text_query.single_mut().sections[1].value = "".to_string();

            match downloaded {
                Ok((id, audio_source)) => {
                    let audio_handle = asset_server.add(audio_source);
                    browser_state.stored_songs.insert(id, audio_handle.clone());
                    let instance_handle = audio.play(audio_handle).paused().handle();
                    commands.spawn(SongPlayer(instance_handle));
                }
                Err(err) => {
                    error!("Song download failed. {}", err);
                }
            };
            commands.remove_resource::<AudioDownloadTask>();
        } else {
            text_query.single_mut().sections[1].value = "Downloading song".to_string();
        };
    }

    if let Some(ref mut level_world) = level_world {
        let task = match **level_world {
            LevelWorld::Pending(ref mut task) => task,
            LevelWorld::World(_) => {
                if level_download_task.is_none()
                    && audio_download_task.is_none()
                    && local_song_handle.is_none()
                {
                    info!("Everything done. Starting execution...");
                    state.set(GameState::Level);
                }
                return;
            }
            _ => return,
        };

        let Some(world) = future::block_on(future::poll_once(task)) else {
            text_query.single_mut().sections[0].value = "Processing level data\n".to_string();
            return;
        };

        text_query.single_mut().sections[0].value = "".to_string();

        let world = match world {
            Ok(world) => world,
            Err(err) => {
                error!("World creation failed. {}", err);
                state.set(GameState::Menu);
                return;
            }
        };

        info!("World created");

        **level_world = LevelWorld::World(Box::new(world));
    }
}

#[cfg(target_arch = "wasm32")]
fn wait_for_creation(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut level_download_task: Option<ResMut<LevelDownloadTask>>,
    mut audio_download_task: Option<ResMut<AudioDownloadTask>>,
    local_song_handle: Option<Res<LocalSongHandle>>,
    mut state: ResMut<NextState<GameState>>,
    cocos2d_frames: Res<Cocos2dFrames>,
    mut text_query: Query<&mut Text, With<PrepareText>>,
    mut browser_state: ResMut<LevelBrowserState>,
    audio: Res<Audio>,
    mut flag: Local<bool>,
) {
    if let Some(ref mut level_download_task) = level_download_task {
        if let Ok(downloaded) = level_download_task.0.try_recv() {
            if !*flag {
                let level_data = match &downloaded {
                    Ok(level_data) => {
                        if level_data
                            .inner_level
                            .as_ref()
                            .map(|inner| inner.len() > 4e+6 as usize)
                            .unwrap_or_default()
                        {
                            text_query.single_mut().sections[0].value =
                                "Processing level data (WARNING: Large level, may take a while...)\n"
                                    .to_string();
                        } else {
                            text_query.single_mut().sections[0].value =
                                "Processing level data\n".to_string();
                        }
                    }
                    Err(err) => {
                        error!("Level download failed. {}", err);
                        state.set(GameState::Menu);
                        return;
                    }
                };

                let (tx, rx) = crossbeam_channel::bounded(1);
                let _ = tx.send(downloaded);
                level_download_task.0 = rx;
                *flag = true;
            } else {
                let level_data = match downloaded {
                    Ok(level_data) => level_data,
                    Err(err) => {
                        error!("Level download failed. {}", err);
                        state.set(GameState::Menu);
                        return;
                    }
                };

                info!("Starting world creation...");

                let cocos2d_frames = cocos2d_frames.clone();
                let low_detail = browser_state.low_detail;

                let start_all = Instant::now();
                let mut start = Instant::now();
                let decompressed = level_data.decompress_inner_level().unwrap().unwrap();
                info!("Decompressing took {:?}", start.elapsed());
                start = Instant::now();
                let parsed = decompressed.parse().unwrap();
                info!("Parsing took {:?}", start.elapsed());
                let world = parsed.create_world(&cocos2d_frames, low_detail);
                info!("Total time: {:?}", start_all.elapsed());

                commands.insert_resource(LevelWorld::World(Box::new(world)));
                commands.remove_resource::<LevelDownloadTask>();
            }
        } else {
            text_query.single_mut().sections[0].value = "Downloading level\n".to_string();
        };
    }

    if let Some(ref local_song_handle) = local_song_handle {
        match asset_server.load_state(local_song_handle.1.clone()) {
            LoadState::Loaded => {
                text_query.single_mut().sections[1].value = "".to_string();
                browser_state
                    .stored_songs
                    .insert(local_song_handle.0.id, local_song_handle.1.clone());
                let instance_handle = audio.play(local_song_handle.1.clone()).paused().handle();
                commands.spawn(SongPlayer(instance_handle));
                commands.remove_resource::<LocalSongHandle>()
            }
            LoadState::Failed => {
                text_query.single_mut().sections[1].value = "".to_string();
                let song_info = local_song_handle.0.clone();
                let (tx, rx) = crossbeam_channel::bounded(1);
                wasm_bindgen_futures::spawn_local(async move {
                    info!("Downloading song {}, ID: {}", song_info.name, song_info.id);
                    let start = Instant::now();
                    let api = DefaultApi::default();
                    let audio_source = api.get_song(song_info.clone()).await;
                    info!("Song download took {:?}", start.elapsed());
                    let _ = tx.send(audio_source.map(|audio_source| (song_info.id, audio_source)));
                });
                commands.insert_resource(AudioDownloadTask(rx));
                commands.remove_resource::<LocalSongHandle>()
            }
            _ => {
                text_query.single_mut().sections[1].value = "Loading local song".to_string();
            }
        }
    }

    if let Some(ref mut audio_download_task) = audio_download_task {
        if let Ok(downloaded) = audio_download_task.0.try_recv() {
            text_query.single_mut().sections[1].value = "".to_string();

            match downloaded {
                Ok((id, audio_source)) => {
                    let audio_handle = asset_server.add(audio_source);
                    browser_state.stored_songs.insert(id, audio_handle.clone());
                    let instance_handle = audio.play(audio_handle).paused().handle();
                    commands.spawn(SongPlayer(instance_handle));
                }
                Err(err) => {
                    error!("Song download failed. {}", err);
                }
            };
            commands.remove_resource::<AudioDownloadTask>();
        } else {
            text_query.single_mut().sections[1].value = "Downloading song".to_string();
        };
    }

    if level_download_task.is_none() && audio_download_task.is_none() && local_song_handle.is_none()
    {
        info!("Everything done. Starting execution...");
        state.set(GameState::Level);
    }
}

fn prepare_cleanup(mut commands: Commands, query: Query<Entity, With<PrepareText>>) {
    commands.remove_resource::<LocalSongHandle>();
    commands.remove_resource::<LevelDownloadTask>();
    commands.remove_resource::<AudioDownloadTask>();
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
