use crate::AlignSelf::Center;
use crate::{GDSaveFile, GameStates, GlobalAssets};
use bevy::app::{App, Plugin};
use bevy::asset::{AssetServer, Assets};
use bevy::ecs::component::Component;
use bevy::hierarchy::{BuildChildren, Children};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::log::info;
use bevy::prelude::{
    default, Button, ButtonBundle, Changed, Color, Commands, Entity, EventReader, Interaction,
    NodeBundle, Query, Res, TextBundle, With,
};
use bevy::text::{Text, TextStyle};
use bevy::ui::{
    AlignSelf, FlexDirection, JustifyContent, Node, Overflow, PositionType, Size, Style, UiColor,
    UiRect, Val,
};

use crate::loaders::gdlevel::GDLevel;
use iyes_loopless::condition::ConditionSet;
use iyes_loopless::prelude::AppLooplessStateExt;

pub(crate) struct LevelSelectStatePlugin;

impl Plugin for LevelSelectStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameStates::LevelSelectState, select_setup)
            .add_exit_system(GameStates::LevelSelectState, select_cleanup)
            .add_system_set(
                ConditionSet::new()
                    .run_in_state(GameStates::LevelSelectState)
                    .with_system(mouse_scroll)
                    .with_system(button_system)
                    .into(),
            );
    }
}

fn select_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    global_assets: Res<GlobalAssets>,
    saves: Res<Assets<GDSaveFile>>,
) {
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
        .insert(SelectMenu)
        .with_children(|parent| {
            parent
                .spawn_bundle(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::ColumnReverse,
                        justify_content: JustifyContent::Center,
                        align_self: Center,
                        size: Size::new(Val::Percent(50.0), Val::Percent(75.0)),
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
                            align_self: Center,
                            margin: UiRect {
                                left: Val::Auto,
                                right: Val::Auto,
                                ..default()
                            },
                            ..default()
                        }),
                    );
                    // List with hidden overflow
                    parent
                        .spawn_bundle(NodeBundle {
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
                                        align_self: Center,
                                        max_size: Size::new(Val::Undefined, Val::Undefined),
                                        ..default()
                                    },
                                    color: Color::NONE.into(),
                                    ..default()
                                })
                                .insert(ScrollingList::default())
                                .with_children(|parent| {
                                    // List items
                                    for level in
                                        saves.get(&global_assets.save_file).unwrap().levels.iter()
                                    {
                                        parent
                                            .spawn_bundle(NodeBundle {
                                                style: Style {
                                                    flex_shrink: 0.,
                                                    size: Size::new(
                                                        Val::Undefined,
                                                        Val::Percent(25.0),
                                                    ),
                                                    margin: UiRect {
                                                        left: Val::Auto,
                                                        right: Val::Auto,
                                                        ..default()
                                                    },
                                                    ..default()
                                                },
                                                color: Color::NONE.into(),
                                                ..default()
                                            })
                                            .with_children(|parent| {
                                                parent.spawn_bundle(
                                                    // Create a TextBundle that has a Text with a list of sections.
                                                    TextBundle::from_section(
                                                        &level.name,
                                                        TextStyle {
                                                            font: asset_server
                                                                .load("fonts/FiraSans-Bold.ttf"),
                                                            font_size: 50.,
                                                            color: Color::WHITE,
                                                        },
                                                    )
                                                    .with_style(Style {
                                                        flex_shrink: 0.,
                                                        size: Size::new(
                                                            Val::Percent(50.),
                                                            Val::Px(50.),
                                                        ),
                                                        margin: UiRect {
                                                            top: Val::Auto,
                                                            bottom: Val::Auto,
                                                            ..default()
                                                        },
                                                        align_self: AlignSelf::FlexStart,
                                                        max_size: Size::new(
                                                            Val::Percent(50.),
                                                            Val::Px(50.),
                                                        ),
                                                        ..default()
                                                    }),
                                                );
                                                parent
                                                    .spawn_bundle(ButtonBundle {
                                                        style: Style {
                                                            flex_shrink: 0.,
                                                            size: Size::new(
                                                                Val::Undefined,
                                                                Val::Px(25.),
                                                            ),
                                                            margin: UiRect {
                                                                top: Val::Auto,
                                                                bottom: Val::Auto,
                                                                ..default()
                                                            },
                                                            ..default()
                                                        },
                                                        ..default()
                                                    })
                                                    .insert(OpenButton {
                                                        level: level.clone(),
                                                    })
                                                    .with_children(|parent| {
                                                        parent.spawn_bundle(
                                                            TextBundle::from_section(
                                                                "Open",
                                                                TextStyle {
                                                                    font: asset_server.load(
                                                                        "fonts/FiraSans-Bold.ttf",
                                                                    ),
                                                                    font_size: 25.0,
                                                                    color: Color::rgb(
                                                                        0.9, 0.9, 0.9,
                                                                    ),
                                                                },
                                                            ),
                                                        );
                                                    });
                                            });
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

#[derive(Component)]
struct OpenButton {
    level: GDLevel,
}

#[derive(Component)]
struct SelectMenu;

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

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.75, 0.35);

fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut UiColor, &OpenButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text>,
) {
    for (interaction, mut color, open) in &mut interaction_query {
        match *interaction {
            Interaction::Clicked => {
                *color = PRESSED_BUTTON.into();
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
            }
        }
    }
}

fn select_cleanup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<Entity, With<SelectMenu>>,
) {
    query.for_each(|entity| {
        commands.entity(entity).despawn();
    });
}
