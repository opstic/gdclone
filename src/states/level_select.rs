use bevy::app::{App, IntoSystemAppConfig, Plugin};
use bevy::asset::{AssetServer, Assets};
use bevy::ecs::component::Component;
use bevy::hierarchy::{BuildChildren, Children, DespawnRecursiveExt};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::text::TextStyle;
use bevy::ui::{
    AlignSelf, BackgroundColor, FlexDirection, JustifyContent, Node, Overflow, Size, Style, UiRect,
    Val,
};

use crate::loaders::gdlevel::SaveFile;
use crate::states::{loading::GlobalAssets, play::LevelIndex, GameState};

pub(crate) struct LevelSelectStatePlugin;

impl Plugin for LevelSelectStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(select_setup.in_schedule(OnEnter(GameState::LevelSelect)))
            .add_system(select_cleanup.in_schedule(OnExit(GameState::LevelSelect)))
            .add_systems((mouse_scroll, button_system).in_set(OnUpdate(GameState::LevelSelect)));
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
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
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
                        justify_content: JustifyContent::SpaceEvenly,
                        align_self: AlignSelf::Center,
                        size: Size::new(Val::Percent(70.0), Val::Percent(80.0)),
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
                                font_size: 55.,
                                color: Color::WHITE,
                            },
                        )
                        .with_style(Style {
                            size: Size::new(Val::Undefined, Val::Px(30.0)),
                            align_self: AlignSelf::Center,
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
                                align_self: AlignSelf::Stretch,
                                size: Size::height(Val::Percent(80.0)),
                                overflow: Overflow::Hidden,
                                ..default()
                            },
                            background_color: Color::rgb(0.10, 0.10, 0.10).into(),
                            ..default()
                        })
                        .with_children(|parent| {
                            // Moving panel
                            parent
                                .spawn((
                                    NodeBundle {
                                        style: Style {
                                            flex_direction: FlexDirection::Column,
                                            flex_grow: 1.0,
                                            max_size: Size::UNDEFINED,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        ..default()
                                    },
                                    ScrollingList::default(),
                                ))
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
                                                    flex_direction: FlexDirection::Row,
                                                    flex_shrink: 0.,
                                                    align_items: AlignItems::Center,
                                                    size: Size::new(
                                                        Val::Percent(99.0),
                                                        Val::Px(100.),
                                                    ),
                                                    margin: UiRect {
                                                        top: Val::Px(5.),
                                                        bottom: Val::Px(5.),
                                                        left: Val::Px(5.),
                                                        right: Val::Px(5.),
                                                        ..default()
                                                    },
                                                    ..default()
                                                },
                                                background_color: Color::rgb(0.12, 0.12, 0.12)
                                                    .into(),
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
                                                            left: Val::Percent(2.5),
                                                            right: Val::Percent(2.5),
                                                            top: Val::Percent(2.5),
                                                            bottom: Val::Percent(2.5),
                                                        },
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
                                                            flex_direction: FlexDirection::Column,
                                                            justify_content: JustifyContent::Center,
                                                            align_items: AlignItems::Center,
                                                            size: Size::new(
                                                                Val::Px(75.),
                                                                Val::Px(50.),
                                                            ),
                                                            margin: UiRect {
                                                                left: Val::Auto,
                                                                right: Val::Percent(2.5),
                                                                top: Val::Percent(2.5),
                                                                bottom: Val::Percent(2.5),
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
                                                                color: Color::rgb(0.8, 0.8, 0.8),
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
            // let max_scroll = (items_height * 2. - panel_height).max(0.);
            let dy = match mouse_wheel_event.unit {
                MouseScrollUnit::Line => mouse_wheel_event.y * 20.,
                MouseScrollUnit::Pixel => mouse_wheel_event.y,
            };
            scrolling_list.position += dy;
            // scrolling_list.position = scrolling_list.position.clamp(-max_scroll, 0.);
            style.position.top = Val::Px(scrolling_list.position);
        }
    }
}

const NORMAL_BUTTON: Color = Color::rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::rgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::rgb(0.35, 0.75, 0.35);

fn button_system(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &OpenButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, button) in &mut interaction_query {
        match *interaction {
            Interaction::Clicked => {
                *color = PRESSED_BUTTON.into();
                info!("Selected button {}", button.level_index);
                commands.insert_resource(LevelIndex {
                    index: button.level_index,
                });
                next_state.set(GameState::Play);
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
