#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use bevy::app::{App, PluginGroup, Startup, Update};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::hierarchy::BuildChildren;
use bevy::input::Input;
use bevy::log::info;
use bevy::prelude::{
    Camera2dBundle, ClearColor, Color, Commands, Component, EventReader, KeyCode, NodeBundle,
    OrthographicProjection, Query, Res, TextBundle, With,
};
use bevy::render::camera::ScalingMode;
use bevy::text::{Text, TextSection, TextStyle};
use bevy::ui::{PositionType, Style, UiRect, Val, ZIndex};
use bevy::utils::default;
use bevy::window::{PresentMode, Window, WindowMode, WindowPlugin, WindowResized};
use bevy::DefaultPlugins;

use crate::asset::AssetPlugin;
use crate::level::LevelPlugin;
use crate::render::RenderPlugins;

mod asset;
mod level;
mod render;
mod utils;

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: concat!("GDClone ", env!("VERSION")).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }),
        FrameTimeDiagnosticsPlugin,
        AssetPlugin,
        LevelPlugin,
        RenderPlugins,
    ));

    app.add_systems(Startup, setup)
        .add_systems(Update, (update_fps, update_scale_factor, toggle_fullscreen));

    app.run()
}

fn setup(mut commands: Commands) {
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

    if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
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
