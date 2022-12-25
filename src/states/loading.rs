use crate::loaders::cocos2d_atlas::Cocos2dAtlas;
use crate::loaders::gdlevel::GDSaveFile;
use crate::loaders::mapping::ObjectMapping;
use crate::GameState;
use bevy::prelude::*;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub(crate) struct LoadingStatePlugin;

impl Plugin for LoadingStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_enter(GameState::Loading).with_system(loading_setup))
            .add_system_set(SystemSet::on_exit(GameState::Loading).with_system(loading_cleanup))
            .add_system_set(
                SystemSet::on_update(GameState::Loading).with_system(check_assets_ready),
            );
    }
}

#[derive(Component)]
struct LoadingText;

#[derive(Resource)]
pub(crate) struct GlobalAssets {
    pub(crate) save_file: Handle<GDSaveFile>,
    pub(crate) texture_mapping: Handle<ObjectMapping>,
    pub(crate) atlas1: Handle<Cocos2dAtlas>,
    pub(crate) atlas2: Handle<Cocos2dAtlas>,
    pub(crate) atlas3: Handle<Cocos2dAtlas>,
    pub(crate) atlas4: Handle<Cocos2dAtlas>,
    pub(crate) atlas5: Handle<Cocos2dAtlas>,
    pub(crate) font: Handle<Font>,
}

#[derive(Resource, Default)]
pub(crate) struct AssetsLoading(Vec<HandleUntyped>);

fn loading_setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut loading: ResMut<AssetsLoading>,
) {
    commands
        .spawn(TextBundle {
            style: Style {
                align_self: AlignSelf::Center,
                ..default()
            },
            text: Text {
                sections: vec![TextSection {
                    value: "Loading...".to_string(),
                    style: TextStyle {
                        font: server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 50.,
                        color: Color::WHITE,
                    },
                }],
                ..default()
            },
            ..default()
        })
        .insert(LoadingText);

    let save_file: Handle<GDSaveFile> = server.load("CCLocalLevels.dat");
    let texture_mapping: Handle<ObjectMapping> = server.load("data/objectTextureMap.json.mapping");
    let atlas1: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet-uhd.plist");
    let atlas2: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet02-uhd.plist");
    let atlas3: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet03-uhd.plist");
    let atlas4: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet04-uhd.plist");
    let atlas5: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheetGlow-uhd.plist");
    let font: Handle<Font> = server.load("fonts/FiraSans-Bold.ttf");

    loading.0.push(save_file.clone_untyped());
    loading.0.push(texture_mapping.clone_untyped());
    loading.0.push(atlas1.clone_untyped());
    loading.0.push(atlas2.clone_untyped());
    loading.0.push(atlas3.clone_untyped());
    loading.0.push(atlas4.clone_untyped());
    loading.0.push(atlas5.clone_untyped());
    loading.0.push(font.clone_untyped());

    commands.insert_resource(GlobalAssets {
        save_file,
        texture_mapping,
        atlas1,
        atlas2,
        atlas3,
        atlas4,
        atlas5,
        font,
    });
}

fn check_assets_ready(
    mut commands: Commands,
    server: Res<AssetServer>,
    loading: Res<AssetsLoading>,
    mut state: ResMut<State<GameState>>,
) {
    use bevy::asset::LoadState;

    match server.get_group_load_state(loading.0.iter().map(|h| h.id)) {
        LoadState::Failed => {}
        LoadState::Loaded => {
            info!("everything loaded");
            commands.remove_resource::<AssetsLoading>();
            state.set(GameState::LevelSelect).unwrap();
        }
        _ => {
            // NotLoaded/Loading: not fully ready yet
        }
    }
}

fn loading_cleanup(mut commands: Commands, query: Query<Entity, With<LoadingText>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
