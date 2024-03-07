use bevy::asset::io::AssetSourceId;
use bevy::asset::{AssetPath, LoadState};
use bevy::prelude::*;

use crate::asset::GlobalAssets;
use crate::state::GameState;

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
}

fn check_assets_ready(
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
    state.set(GameState::Menu);
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
