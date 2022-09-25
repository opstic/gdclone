use bevy::app::{App, Plugin};
use bevy::asset::AssetServer;
use bevy::ecs::system::Command;
use bevy::hierarchy::{BuildChildren, Children};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::{Color, Commands, default, EventReader, ImageBundle, NodeBundle, Query, Res, TextBundle};
use bevy::text::TextStyle;
use bevy::ui::{AlignItems, AlignSelf, FlexDirection, JustifyContent, Node, Overflow, PositionType, Size, Style, UiRect, Val};
use bevy_editor_pls::egui::Ui;
use iyes_loopless::prelude::AppLooplessStateExt;
use bevy::ecs::component::Component;
use iyes_loopless::condition::ConditionSet;
use crate::AlignSelf::Center;
use crate::GameStates;
use crate::KeyCode::C;

pub(crate) struct LevelSelectStatePlugin;

impl Plugin for LevelSelectStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameStates::LevelSelectState, select_setup)
            .add_exit_system(GameStates::LevelSelectState, select_cleanup)
        .add_system_set(
            ConditionSet::new()
                .run_in_state(GameStates::LevelSelectState)
                .with_system(mouse_scroll)
                .into(),
        );
    }
}


fn select_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            color: Color::NONE.into(),
            ..default()
        })
        .with_children(|parent| {
            // right vertical fill
            parent
                .spawn_bundle(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::ColumnReverse,
                        justify_content: JustifyContent::Center,
                        align_self:Center,
                        size: Size::new(Val::Px(200.0), Val::Percent(100.0)),
                        ..default()
                    },
                     color: Color::rgb(0.15, 0.15, 0.15).into(),
                    ..default()
                })
                .with_children(|parent| {
                    // Title
                    parent.spawn_bundle(
                        TextBundle::from_section(
                            "Loaded Levels",
                            TextStyle {
                                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 25.,
                                color: Color::WHITE,
                            },
                        )
                            .with_style(Style {
                                size: Size::new(Val::Undefined, Val::Px(30.0)),
                                align_self:Center,
                                margin: UiRect {
                                    left: Val::Auto,
                                    right: Val::Auto,
                                    ..default()
                                },
                                ..default()
                            }),
                    );
                    // List with hidden overflow
                    parent.spawn_bundle(
                        NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::ColumnReverse,
                                align_self: Center,
                                size: Size::new(Val::Percent(100.0), Val::Percent(50.0)),
                                overflow: Overflow::Hidden,
                                ..default()
                            },
                            color: Color::rgb(0.10, 0.10, 0.10).into(),
                            ..default()
                        })
                        .with_children(|parent| {
                            // Moving panel
                            parent
                                .spawn_bundle(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::ColumnReverse,
                                        flex_grow: 1.0,
                                        align_self:Center,
                                        max_size: Size::new(Val::Undefined, Val::Undefined),
                                        ..default()
                                    },
                                    color: Color::NONE.into(),
                                    ..default()
                                })
                                .insert(ScrollingList::default())
                                .with_children(|parent| {
                                    // List items
                                    for i in 0..30 {
                                        parent.spawn_bundle(
                                            TextBundle::from_section(
                                                format!("Item {i}"),
                                                TextStyle {
                                                    font: asset_server
                                                        .load("fonts/FiraSans-Bold.ttf"),
                                                    font_size: 20.,
                                                    color: Color::WHITE,
                                                },
                                            )
                                                .with_style(Style {
                                                    flex_shrink: 0.,
                                                    size: Size::new(Val::Undefined, Val::Px(20.)),
                                                    align_self:Center,
                                                    margin: UiRect {
                                                        left: Val::Auto,
                                                        right: Val::Auto,
                                                        ..default()
                                                    },
                                                    ..default()
                                                }),
                                        );
                                    }
                                });
                        });
                });
        });
}

#[derive(Component, Default)]
struct ScrollingList {
    position: f32,
}

fn mouse_scroll(
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut query_list: Query<(&mut ScrollingList, &mut Style, &Children, &Node)>,
    query_item: Query<&Node>,
) {
    for mouse_wheel_event in mouse_wheel_events.iter() {
        for (mut scrolling_list, mut style, children, uinode) in &mut query_list {
            let items_height: f32 = children
                .iter()
                .map(|entity| query_item.get(*entity).unwrap().size.y)
                .sum();
            let panel_height = uinode.size.y;
            let max_scroll = (items_height - panel_height).max(0.);
            let dy = match mouse_wheel_event.unit {
                MouseScrollUnit::Line => mouse_wheel_event.y * 20.,
                MouseScrollUnit::Pixel => mouse_wheel_event.y,
            };
            scrolling_list.position += dy;
            scrolling_list.position = scrolling_list.position.clamp(-max_scroll, 0.);
            style.position.top = Val::Px(scrolling_list.position);
        }
    }
}

fn select_cleanup(mut commands: Commands, asset_server: Res<AssetServer>){}
