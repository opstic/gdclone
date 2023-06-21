use crate::loaders::cocos2d_atlas::Cocos2dAtlas;
use crate::loaders::gdlevel::SaveFile;
use crate::GameState;
use bevy::prelude::*;

pub(crate) struct LoadingStatePlugin;

impl Plugin for LoadingStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(loading_setup.in_schedule(OnEnter(GameState::Loading)))
            .add_system(loading_cleanup.in_schedule(OnExit(GameState::Loading)))
            .add_system(check_assets_ready.in_set(OnUpdate(GameState::Loading)))
            .add_system(update_asset_text.in_set(OnUpdate(GameState::Loading)));
    }
}

#[derive(Component)]
struct LoadingText;

#[derive(Component)]
struct ListText;

#[derive(Resource)]
pub(crate) struct GlobalAssets {
    pub(crate) save_file: Handle<SaveFile>,
    pub(crate) atlas1: Handle<Cocos2dAtlas>,
    pub(crate) atlas2: Handle<Cocos2dAtlas>,
    pub(crate) atlas3: Handle<Cocos2dAtlas>,
    pub(crate) atlas4: Handle<Cocos2dAtlas>,
    pub(crate) atlas5: Handle<Cocos2dAtlas>,
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
                    value: "".to_string(),
                    style: TextStyle {
                        font: server.load("fonts/FiraMono-Medium.ttf"),
                        font_size: 20.,
                        color: Color::WHITE,
                    },
                }],
                ..default()
            },
            ..default()
        })
        .insert(ListText)
        .insert(LoadingText);

    let save_file: Handle<SaveFile> = server.load("CCLocalLevels.dat");
    let atlas1: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet-uhd.plist");
    let atlas2: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet02-uhd.plist");
    let atlas3: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet03-uhd.plist");
    let atlas4: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheet04-uhd.plist");
    let atlas5: Handle<Cocos2dAtlas> = server.load("Resources/GJ_GameSheetGlow-uhd.plist");

    loading.0.push(save_file.clone_untyped());
    loading.0.push(atlas1.clone_untyped());
    loading.0.push(atlas2.clone_untyped());
    loading.0.push(atlas3.clone_untyped());
    loading.0.push(atlas4.clone_untyped());
    loading.0.push(atlas5.clone_untyped());

    commands.insert_resource(GlobalAssets {
        save_file,
        atlas1,
        atlas2,
        atlas3,
        atlas4,
        atlas5,
    });
}

fn check_assets_ready(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut loading: ResMut<AssetsLoading>,
    mut state: ResMut<NextState<GameState>>,
) {
    use bevy::asset::LoadState;

    loading
        .0
        .retain(|h| server.get_load_state(h) != LoadState::Loaded);

    if loading.0.is_empty() {
        info!("Everything loaded");
        commands.remove_resource::<AssetsLoading>();
        state.set(GameState::LevelSelect);
    }
}

fn update_asset_text(
    server: Res<AssetServer>,
    loading: Res<AssetsLoading>,
    mut query: Query<&mut Text, With<ListText>>,
) {
    for mut text in query.iter_mut() {
        let names: String = loading
            .0
            .iter()
            .map(|h| {
                server
                    .get_handle_path(h)
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

fn loading_cleanup(mut commands: Commands, query: Query<Entity, With<LoadingText>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
