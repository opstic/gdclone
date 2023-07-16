// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use std::time::Duration;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::sprite::SpritePlugin;
use bevy::window::{PresentMode, WindowMode, WindowResizeConstraints};
use bevy::winit::{WinitSettings, WinitWindows};
use winit::window::Icon;

use compressed_image::CompressedImagePlugin;
use discord::DiscordPlugin;
use level::LevelPlugin;
use loader::AssetLoaderPlugin;
use multi_asset_io::MultiAssetIoPlugin;
use render::sprite::CustomSpritePlugin;
use state::{loading::AssetsLoading, play::Player, GameState, StatePlugins};
use transform::CustomTransformPlugin;

mod compressed_image;
mod discord;
mod level;
mod loader;
mod multi_asset_io;
mod par_iter_many;
mod render;
mod state;
mod transform;
mod utils;

fn main() {
    let mut app = App::new();
    app.insert_resource(WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::ReactiveLowPower {
            max_wait: Duration::from_millis(100),
        },
        ..default()
    })
    .insert_resource(Msaa::Off)
    .insert_resource(AssetsLoading::default())
    .add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resize_constraints: WindowResizeConstraints {
                        // well if you are willing to play at such horrendous resolution here you go
                        min_width: 128.,
                        min_height: 72.,
                        ..default()
                    },
                    title: "GDClone".to_string(),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            })
            .disable::<TransformPlugin>()
            .add_before::<TransformPlugin, CustomTransformPlugin>(CustomTransformPlugin)
            .disable::<SpritePlugin>()
            .add_before::<SpritePlugin, CustomSpritePlugin>(CustomSpritePlugin)
            .add_before::<AssetPlugin, MultiAssetIoPlugin>(MultiAssetIoPlugin),
    )
    .add_plugins((
        DiscordPlugin,
        FrameTimeDiagnosticsPlugin,
        CompressedImagePlugin,
        AssetLoaderPlugin,
        LevelPlugin,
    ))
    .add_state::<GameState>()
    .add_plugins(StatePlugins)
    .add_systems(Startup, setup)
    .add_systems(Update, (update_fps, toggle_fullscreen))
    .run();
}

#[derive(Component)]
struct FpsText;

const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/branding/icon.png"
));

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    window_entities: Query<Entity, With<Window>>,
    winit_windows: NonSend<WinitWindows>,
) {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(ICON).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap();
    for entity in &window_entities {
        let winit_window = winit_windows.get_window(entity).unwrap();
        winit_window.set_window_icon(Some(icon.clone()));
    }

    commands
        .spawn(Camera2dBundle {
            projection: OrthographicProjection {
                scaling_mode: ScalingMode::AutoMin {
                    min_width: 1280.,
                    min_height: 720.,
                },
                ..default()
            },
            ..default()
        })
        .insert(Player(Vec2::ZERO));
    commands
        .spawn(TextBundle {
            style: Style {
                position_type: PositionType::Absolute,
                align_self: AlignSelf::FlexStart,
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

fn update_fps(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in query.iter_mut() {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = fps.average() {
                text.sections[1].value = average.trunc().to_string();
            }
        }
    }
}

fn toggle_fullscreen(input: Res<Input<KeyCode>>, mut windows: Query<&mut Window>) {
    if input.just_pressed(KeyCode::F11) {
        let mut window = windows.single_mut();

        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::Fullscreen,
            WindowMode::Fullscreen => WindowMode::BorderlessFullscreen,
            WindowMode::BorderlessFullscreen => WindowMode::SizedFullscreen,
            WindowMode::SizedFullscreen => WindowMode::Windowed,
        };

        info!("Switching window mode to {:?}", window.mode);
    }
}
