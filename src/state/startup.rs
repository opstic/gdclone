use crate::api::{DefaultApi, ServerApi};
use bevy::asset::io::AssetSourceId;
use bevy::asset::{AssetPath, LoadState};
use bevy::prelude::*;
use bevy::utils::HashMap;
use instant::Instant;
use wasm_bindgen::JsCast;

use crate::asset::GlobalAssets;
use crate::level::{LevelInfo, SongInfo};
use crate::state::menu::LevelBrowserState;
use crate::state::prepare::LevelToDownload;
use crate::state::GameState;
use crate::utils::str_to_bool;

pub(crate) struct StartupStatePlugin;

impl Plugin for StartupStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Startup), startup_setup)
            .add_systems(OnExit(GameState::Startup), startup_cleanup)
            .add_systems(
                Update,
                (check_assets_ready, update_asset_text).run_if(in_state(GameState::Startup)),
            );
    }
}

#[derive(Component)]
struct StartupEntity;

#[derive(Component)]
struct ListText;

fn startup_setup(mut commands: Commands, server: Res<AssetServer>) {
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
        .insert(StartupEntity)
        .with_children(|parent| {
            parent
                .spawn(TextBundle {
                    style: Style {
                        width: Val::Percent(80.),
                        height: Val::Auto,
                        ..default()
                    },
                    text: Text {
                        sections: vec![TextSection {
                            value: "".to_string(),
                            style: TextStyle {
                                font_size: 20.,
                                color: Color::WHITE,
                                ..default()
                            },
                        }],
                        ..default()
                    },
                    ..default()
                })
                .insert(ListText);
        });

    let source = AssetSourceId::from("resources");

    commands.insert_resource(GlobalAssets {
        assets: vec![
            server.load(AssetPath::from("GJ_GameSheet-uhd.plist").with_source(source.clone())),
            server.load(AssetPath::from("GJ_GameSheet02-uhd.plist").with_source(source.clone())),
            server.load(AssetPath::from("GJ_GameSheet03-uhd.plist").with_source(source.clone())),
            server.load(AssetPath::from("GJ_GameSheet04-uhd.plist").with_source(source.clone())),
            server.load(AssetPath::from("GJ_GameSheetGlow-uhd.plist").with_source(source)),
        ],
    });

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsValue;
        let global = js_sys::global();
        if let Ok(window) = js_sys::Reflect::get(&global, &JsValue::from_str("Window")) {
            if !window.is_undefined() {
                let window = global.dyn_into::<web_sys::Window>().unwrap();
                if let Some(url) = window
                    .location()
                    .href()
                    .ok()
                    .and_then(|href| url::Url::parse(&href).ok())
                {
                    let mut parsed_query = HashMap::new();
                    parsed_query.extend(url.query_pairs());
                    let mut options = LevelBrowserState::default();
                    if let Some(use_song) = parsed_query.get("use_song") {
                        options.use_song = str_to_bool(use_song);
                    }
                    if let Some(low_detail) = parsed_query.get("low_detail") {
                        options.low_detail = str_to_bool(low_detail);
                    }
                    if let Some(start_paused) = parsed_query.get("start_paused") {
                        options.start_paused = str_to_bool(start_paused);
                    }
                    commands.insert_resource(options);
                    if let Some(preload) = parsed_query.get("preload") {
                        let preload = preload.to_string();
                        info!("Recieved query to preload: {}", preload);
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        wasm_bindgen_futures::spawn_local(async move {
                            let api = DefaultApi::default();
                            let _ = tx.send(api.search_levels(preload).await);
                        });
                        commands.insert_resource(SearchForId(rx));
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
struct SearchForId(
    crossbeam_channel::Receiver<Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error>>,
);

fn check_assets_ready(
    #[cfg(target_arch = "wasm32")] preload_search: Option<Res<SearchForId>>,
    #[cfg(target_arch = "wasm32")] mut browser_state: ResMut<LevelBrowserState>,
    #[cfg(target_arch = "wasm32")] mut commands: Commands,
    server: Res<AssetServer>,
    assets: ResMut<GlobalAssets>,
    mut state: ResMut<NextState<GameState>>,
) {
    if assets
        .assets
        .iter()
        .any(|h| server.load_state(h.clone()) != LoadState::Loaded)
    {
        return;
    }

    info!("All resources loaded.");

    #[cfg(not(target_arch = "wasm32"))]
    state.set(GameState::Menu);

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(search) = preload_search {
            if let Ok(recv) = search.0.try_recv() {
                let (level_infos, song_infos) = match recv {
                    Ok(result) => result,
                    Err(error) => {
                        error!("Error while making request: {}", error);
                        state.set(GameState::Menu);
                        return;
                    }
                };

                browser_state.song_infos.extend(song_infos);

                if level_infos.is_empty() {
                    error!("Failed to find level with specified preload query");
                    state.set(GameState::Menu);
                }

                if level_infos.len() > 1 {
                    warn!("Server returned more than one result for the specified query. Selecting the first result")
                }

                commands.insert_resource(LevelToDownload(level_infos[0].clone()));
                state.set(GameState::Prepare);
            }
        } else {
            state.set(GameState::Menu);
        }
    }
}

fn update_asset_text(
    server: Res<AssetServer>,
    loading: Res<GlobalAssets>,
    mut query: Query<&mut Text, With<ListText>>,
) {
    for mut text in query.iter_mut() {
        let names: String = loading
            .assets
            .iter()
            .map(|h| {
                server
                    .get_path(h.clone())
                    .unwrap()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
                    + ": "
                    + &*format!("{:?}", server.get_load_state(h))
            })
            .collect::<Vec<String>>()
            .join("\n");
        text.sections[0].value = names;
    }
}

fn startup_cleanup(mut commands: Commands, query: Query<Entity, With<StartupEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
