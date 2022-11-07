use crate::GameState;
use bevy::prelude::*;
use iyes_loopless::prelude::AppLooplessStateExt;

pub(crate) struct LoadingStatePlugin;

impl Plugin for LoadingStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameState::Loading, loading_setup)
            .add_exit_system(GameState::Loading, loading_cleanup);
    }
}

#[derive(Component)]
struct LoadingText;

fn loading_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::Center,
                ..default()
            },
            text: Text {
                sections: vec![TextSection {
                    value: "Loading...".to_string(),
                    style: TextStyle {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 50.,
                        color: Color::WHITE,
                    },
                }],
                ..default()
            },
            ..default()
        })
        .insert(LoadingText);
}

fn loading_cleanup(mut commands: Commands, query: Query<Entity, With<LoadingText>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
