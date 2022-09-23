#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResizeConstraints, WindowResized};
use bevy::winit::WinitSettings;
use bevy_asset_loader::prelude::*;
use bevy_editor_pls::EditorPlugin;
use bevy_kira_audio::AudioPlugin;
use bevy_tweening::*;
use bevy_ui_navigation::DefaultNavigationPlugins;
use iyes_loopless::prelude::AppLooplessStateExt;
use iyes_progress::ProgressPlugin;
use std::string::String;
use std::time::Duration;

mod loaders;
mod states;

use loaders::{gdlevel::GDLevel, AssetLoaderPlugin};
use states::{GameStates, StatePlugins};

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            resize_constraints: WindowResizeConstraints {
                // well if you are willing to play at such horrendous resolution here you go
                min_width: 128.,
                min_height: 72.,
                ..default()
            },
            title: "GDClone".to_string(),
            present_mode: PresentMode::Immediate,
            ..default()
        })
        .insert_resource(WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::ReactiveLowPower {
                max_wait: Duration::from_millis(100),
            },
            ..default()
        })
        .add_loopless_state(GameStates::LoadingState)
        .add_plugin(ProgressPlugin::new(GameStates::LoadingState))
        .add_loading_state(
            LoadingState::new(GameStates::LoadingState)
                .continue_to_state(GameStates::PlayState)
                .with_collection::<LevelAssets>(),
        )
        .add_plugins(DefaultPlugins)
        .add_plugin(AudioPlugin)
        // .add_plugin(EditorPlugin)
        .add_plugins(DefaultNavigationPlugins)
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(TweeningPlugin)
        .add_plugin(AssetLoaderPlugin)
        .add_plugins(StatePlugins)
        .add_startup_system(setup)
        .add_system(update_fps)
        .add_system(handle_resize)
        .run();
}

#[derive(AssetCollection)]
struct LevelAssets {
    #[asset(path = "CCLocalLevels.dat")]
    level: Handle<GDLevel>,
}

#[derive(Component)]
struct FpsText;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(Camera2dBundle::default());
    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                ..default()
            },
            text: Text {
                sections: vec![
                    TextSection {
                        value: "FPS: ".to_string(),
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 12.5,
                            color: Color::WHITE,
                        },
                    },
                    TextSection {
                        value: "".to_string(),
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                            font_size: 12.5,
                            color: Color::GOLD,
                        },
                    },
                ],
                ..default()
            },
            ..default()
        })
        .insert(FpsText);
}

fn update_fps(diagnostics: Res<Diagnostics>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in query.iter_mut() {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = fps.average() {
                text.sections[1].value = average.trunc().to_string();
            }
        }
    }
}

fn handle_resize(mut windows: ResMut<Windows>, mut resize_events: EventReader<WindowResized>) {
    for event in resize_events.iter() {
        match windows.get_mut(event.id) {
            Some(window) => {
                let scale_factor = f32::min(
                    window.physical_width() as f32 / window.requested_width(),
                    window.physical_height() as f32 / window.requested_height(),
                ) as f64;
                if scale_factor != 0.0 {
                    window.update_scale_factor_from_backend(scale_factor);
                }
            }
            None => unreachable!("Bevy should have handled ghost window events for us"),
        }
    }
}
