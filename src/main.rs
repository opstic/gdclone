#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use bevy::app::{App, PluginGroup, Startup, Update};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::hierarchy::BuildChildren;
use bevy::prelude::{
    Camera2dBundle, ClearColor, Color, Commands, Component, NodeBundle, Query, Res, TextBundle,
    With,
};
use bevy::text::{Text, TextSection, TextStyle};
use bevy::ui::{PositionType, Style, UiRect, Val, ZIndex};
use bevy::utils::default;
use bevy::window::{PresentMode, Window, WindowPlugin};
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
        .add_systems(Update, update_fps);

    app.run()
}

fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.tonemapping = Tonemapping::None;
    camera_bundle.deband_dither = DebandDither::Disabled;
    camera_bundle.projection.scale = 1.;
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
