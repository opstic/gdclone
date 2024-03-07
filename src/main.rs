#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;

use bevy::app::{App, PluginGroup, Startup, Update};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceBuilders};
use bevy::core::{TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::hierarchy::BuildChildren;
use bevy::input::ButtonInput;
use bevy::log::info;
use bevy::prelude::{
    Camera2dBundle, ClearColor, Color, Commands, Component, Entity, EventReader, KeyCode,
    NodeBundle, NonSend, OrthographicProjection, Query, Res, TextBundle, With,
};
use bevy::render::camera::ScalingMode;
use bevy::text::{Text, TextSection, TextStyle};
use bevy::ui::{PositionType, Style, UiRect, Val, ZIndex};
use bevy::utils::default;
use bevy::window::{PresentMode, Window, WindowMode, WindowPlugin, WindowResized};
use bevy::winit::WinitWindows;
use bevy::DefaultPlugins;
use bevy_egui::EguiPlugin;
use bevy_kira_audio::AudioPlugin;
use directories::{BaseDirs, ProjectDirs};
use native_dialog::{FileDialog, MessageDialog, MessageType};
use serde::{Deserialize, Serialize};
use steamlocate::SteamDir;
use winit::window::Icon;

use crate::asset::AssetPlugin;
use crate::render::RenderPlugins;
use crate::state::StatePlugin;

mod api;
mod asset;
mod level;
mod render;
mod state;
mod utils;

fn main() {
    let mut app = App::new();

    setup_asset_dirs(&mut app);

    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: concat!("GDClone ", env!("VERSION")).into(),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions {
                    compute: TaskPoolThreadAssignmentPolicy {
                        min_threads: 2,
                        max_threads: usize::MAX,
                        percent: 1.0,
                    },
                    ..default()
                },
            }),
        FrameTimeDiagnosticsPlugin,
        AudioPlugin,
        EguiPlugin,
        AssetPlugin,
        RenderPlugins,
        StatePlugin,
    ));

    app.add_systems(Startup, setup)
        .add_systems(Update, (update_fps, update_scale_factor, toggle_fullscreen));

    app.run()
}

const GEOMETRY_DASH_APP_ID: u32 = 322170;

#[derive(Serialize, Deserialize, Debug, Default)]
struct PathConfig {
    gd_path: String,
    gd_data_path: String,
}

fn setup_asset_dirs(app: &mut App) {
    let project_dirs = ProjectDirs::from("dev", "Opstic", "GDClone").unwrap();
    let base_dirs = BaseDirs::new().unwrap();

    std::fs::create_dir_all(project_dirs.config_local_dir()).unwrap();

    let config_path = project_dirs.config_local_dir().join("path_config.json");

    let mut path_config = if let Ok(config_file) = File::open(config_path.clone()) {
        serde_json::from_reader(BufReader::new(config_file)).unwrap_or_default()
    } else {
        PathConfig::default()
    };

    let config_gd_path = PathBuf::from(&path_config.gd_path);

    let gd_path = if config_gd_path.join("Resources").is_dir() {
        config_gd_path
    } else if let Some(path) = match SteamDir::locate() {
        Ok(mut steam_dir) => match steam_dir.find_app(GEOMETRY_DASH_APP_ID) {
            Ok(app) => app.map(|(app, library)| library.resolve_app_dir(&app)),
            Err(_) => None,
        },
        Err(_) => None,
    } {
        path
    } else {
        MessageDialog::new()
            .set_type(MessageType::Error)
            .set_title("Error when locating")
            .set_text("Cannot locate Geometry Dash. Please select the install directory manually.")
            .show_alert()
            .unwrap();

        let mut gd_path = PathBuf::new();

        while !gd_path.join("Resources").is_dir() {
            let selected_path = FileDialog::new().show_open_single_dir().unwrap();
            if let Some(selected_path) = selected_path {
                let resources_path = selected_path.join("Resources");
                if resources_path.is_dir() {
                    gd_path = selected_path;
                } else {
                    MessageDialog::new()
                        .set_type(MessageType::Error)
                        .set_title("Error when locating")
                        .set_text(
                            "This directory does not contain the necessary files. Please select the correct directory.",
                        )
                        .show_alert()
                        .unwrap();
                }
            } else {
                std::process::exit(0);
            }
        }
        gd_path
    };

    path_config.gd_path = gd_path.into_os_string().into_string().unwrap();

    let config_gd_data_path = PathBuf::from(path_config.gd_data_path);

    let gd_data_path = if config_gd_data_path.join("CCLocalLevels.dat").is_file() {
        config_gd_data_path
    } else if base_dirs
        .data_local_dir()
        .join("GeometryDash/CCLocalLevels.dat")
        .is_file()
    {
        base_dirs.data_local_dir().join("GeometryDash")
    } else {
        MessageDialog::new()
            .set_type(MessageType::Error)
            .set_title("Error when locating")
            .set_text(
                "Cannot locate Geometry Dash's data directory. Please select the data directory manually.",
            )
            .show_alert()
            .unwrap();

        let mut gd_data_path = PathBuf::new();

        while !gd_data_path.join("CCLocalLevels.dat").is_file() {
            let selected_path = FileDialog::new().show_open_single_dir().unwrap();
            if let Some(selected_path) = selected_path {
                let levels_path = selected_path.join("CCLocalLevels.dat");
                if levels_path.is_file() {
                    gd_data_path = selected_path;
                } else {
                    MessageDialog::new()
                        .set_type(MessageType::Error)
                        .set_title("Error when locating")
                        .set_text(
                            "This directory does not contain the necessary files. Please select the correct directory.",
                        )
                        .show_alert()
                        .unwrap();
                }
            } else {
                break;
            }
        }
        gd_data_path
    };

    path_config.gd_data_path = gd_data_path.into_os_string().into_string().unwrap();

    let mut config_file = File::create(config_path).unwrap();

    config_file
        .write_all(
            serde_json::to_string_pretty(&path_config)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

    let mut sources = app
        .world
        .get_resource_or_insert_with::<AssetSourceBuilders>(default);
    sources.insert(
        "resources",
        AssetSourceBuilder::platform_default(&(path_config.gd_path + "/Resources"), None),
    );
    sources.insert(
        "data",
        AssetSourceBuilder::platform_default(&path_config.gd_data_path, None),
    );
}

const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/branding/icon.png"
));

fn setup(
    mut commands: Commands,
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

    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.tonemapping = Tonemapping::None;
    camera_bundle.deband_dither = DebandDither::Disabled;
    camera_bundle.projection.scale = 1.;
    camera_bundle.projection.near = -10000.;
    commands.spawn(camera_bundle);

    let fps_container = commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            z_index: ZIndex::Global(i32::MAX),
            background_color: Color::BLACK.with_a(0.5).into(),
            ..default()
        })
        .id();

    let fps_text = commands
        .spawn(TextBundle::from_sections([
            TextSection::new(
                "FPS: ",
                TextStyle {
                    font_size: 15.,
                    ..default()
                },
            ),
            TextSection::new(
                "",
                TextStyle {
                    font_size: 15.,
                    ..default()
                },
            ),
        ]))
        .insert(FpsText)
        .id();

    commands.entity(fps_container).add_child(fps_text);

    commands.insert_resource(ClearColor(Color::BLACK));
}

#[derive(Component)]
struct FpsText;

fn update_fps(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text, With<FpsText>>) {
    let mut text = query.single_mut();

    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(average) = fps.average() {
            text.sections[1].value = average.trunc().to_string();
        }
    };
}

fn update_scale_factor(
    mut projections: Query<&mut OrthographicProjection>,
    mut resize_events: EventReader<WindowResized>,
) {
    for resize_event in resize_events.read() {
        let width_scale_factor: f64 = resize_event.width as f64 / 568.;
        let height_scale_factor: f64 = resize_event.height as f64 / 320.;

        let scale_factor = width_scale_factor.min(height_scale_factor);
        if scale_factor == 1. {
            continue;
        }

        for mut projection in &mut projections {
            projection.scaling_mode = ScalingMode::WindowSize(scale_factor as f32);
        }
    }
}

fn toggle_fullscreen(input: Res<ButtonInput<KeyCode>>, mut windows: Query<&mut Window>) {
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
