#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResizeConstraints, WindowResized};
use bevy::winit::WinitSettings;
use bevy_kira_audio::AudioPlugin;
use bevy_tweening::*;
use bevy_ui_navigation::DefaultNavigationPlugins;
use std::time::Duration;

mod gdlevel;

mod states;
use crate::gdlevel::{GDLevel, GDLevelLoader};
use states::GameStates;

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
        .insert_resource(Msaa::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(AudioPlugin)
        .add_plugins(DefaultNavigationPlugins)
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(TweeningPlugin)
        .add_startup_system(setup)
        .add_system(update_fps)
        .add_system(handle_resize)
        .add_asset::<GDLevel>()
        .init_asset_loader::<GDLevelLoader>()
        .run();
}

#[derive(Component)]
struct FpsText;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(Camera2dBundle::default());
    commands.spawn_bundle(SpriteBundle {
        sprite: Sprite {
            color: Color::rgb(1.0, 0.25, 0.75),
            custom_size: Some(Vec2::new(1280.0, 720.0)),
            ..default()
        },
        ..default()
    });
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
    let _: Handle<GDLevel> = asset_server.load("Resources/CCGameManager.dat");
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
            None => warn!("Window {:?} does not exist", event.id),
        }
    }
}
