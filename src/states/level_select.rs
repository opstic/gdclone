use crate::AlignSelf::Center;
use crate::GameState;
use bevy::app::{App, Plugin};
use bevy::asset::{AssetServer, Assets};
use bevy::ecs::component::Component;
use bevy::hierarchy::{BuildChildren, Children, DespawnRecursiveExt};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::{
    default, Button, ButtonBundle, Changed, Color, Commands, Entity, EventReader, Interaction,
    NodeBundle, Query, Res, ResMut, State, SystemSet, TextBundle, With,
};
use bevy::text::TextStyle;
use bevy::ui::{
    AlignSelf, BackgroundColor, FlexDirection, JustifyContent, Node, Overflow, Size, Style, UiRect,
    Val,
};
use crate::loaders::gdlevel::SaveFile;

use super::loading::GlobalAssets;
use super::play::LevelIndex;

pub(crate) struct LevelSelectStatePlugin;

impl Plugin for LevelSelectStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_enter(GameState::LevelSelect).with_system(select_setup))
            .add_system_set(SystemSet::on_exit(GameState::LevelSelect).with_system(select_cleanup))
            .add_system_set(
                SystemSet::on_update(GameState::LevelSelect)
                    .with_system(mouse_scroll)
                    .with_system(button_system),
            );
    }
}

fn select_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    global_assets: Res<GlobalAssets>,
    saves: Res<Assets<SaveFile>>,
) {
    commands
        .spawn(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            background_color: Color::NONE.into(),
            ..default()
        })
        .insert(SelectMenu)
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_self: Center,
                        size: Size::new(Val::Percent(50.0), Val::Percent(75.0)),
                        ..default()
                    },
                    background_color: Color::rgb(0.15, 0.15, 0.15).into(),
                    ..default()
                })
                .with_children(|parent| {
                    // Title
                    parent.spawn(
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
                        .spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Column,
                                align_self: Center,
                                size: Size::new(Val::Percent(100.0), Val::Percent(50.0)),
                                overflow: Overflow::Hidden,
                                ..default()
                            },
                            background_color: Color::rgb(0.10, 0.10, 0.10).into(),
                            ..default()
                        })
                        .with_children(|parent| {
                            // Moving panel
                            parent
                                .spawn(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::Column,
                                        flex_grow: 1.0,
                                        align_self: Center,
                                        max_size: Size::new(Val::Undefined, Val::Undefined),
                                        ..default()
                                    },
                                    background_color: Color::NONE.into(),
                                    ..default()
                                })
                                .insert(ScrollingList::default())
                                .with_children(|parent| {
                                    // List items
                                    for (index, level) in saves
                                        .get(&global_assets.save_file)
                                        .unwrap()
                                        .levels
                                        .iter()
                                        .enumerate()
                                    {
                                        parent
                                            .spawn(NodeBundle {
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
                                                background_color: Color::NONE.into(),
                                                ..default()
                                            })
                                            .with_children(|parent| {
                                                parent.spawn(
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
                                                        align_self: AlignSelf::FlexEnd,
                                                        max_size: Size::new(
                                                            Val::Percent(50.),
                                                            Val::Px(50.),
                                                        ),
                                                        ..default()
                                                    }),
                                                );
                                                parent
                                                    .spawn(ButtonBundle {
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
                                                    .insert(OpenButton { level_index: index })
                                                    .with_children(|parent| {
                                                        parent.spawn(TextBundle::from_section(
                                                            "Open",
                                                            TextStyle {
                                                                font: asset_server.load(
                                                                    "fonts/FiraSans-Bold.ttf",
                                                                ),
                                                                font_size: 25.0,
                                                                color: Color::rgb(0.9, 0.9, 0.9),
                                                            },
                                                        ));
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
    level_index: usize,
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
                .map(|entity| query_item.get(*entity).unwrap().size().y)
                .sum();
            let panel_height = uinode.size().y;
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
    mut commands: Commands,
    mut state: ResMut<State<GameState>>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &OpenButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, button) in &mut interaction_query {
        match *interaction {
            Interaction::Clicked => {
                *color = PRESSED_BUTTON.into();
                commands.insert_resource(LevelIndex {
                    index: button.level_index,
                });
                state.set(GameState::Play).unwrap()
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

fn select_cleanup(mut commands: Commands, query: Query<Entity, With<SelectMenu>>) {
    query.for_each(|entity| commands.entity(entity).despawn_recursive());
}
