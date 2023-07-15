use bevy::app::{App, IntoSystemAppConfig, Plugin};
use bevy::asset::{AssetServer, Assets};
use bevy::ecs::component::Component;
use bevy::hierarchy::{BuildChildren, DespawnRecursiveExt};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::text::TextStyle;
use bevy::ui::{
    AlignSelf, BackgroundColor, FlexDirection, JustifyContent, Node, Overflow, Size, Style, UiRect,
    Val,
};
use discord_sdk::{activity, activity::ActivityBuilder};

use crate::discord::CurrentDiscordActivity;
use crate::loader::gdlevel::SaveFile;
use crate::state::{loading::GlobalAssets, play::LevelIndex, GameState};

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
    mut discord_activity: ResMut<CurrentDiscordActivity>,
) {
    discord_activity.0 = ActivityBuilder::default()
        .details(format!(
            "{} levels loaded",
            saves.get(&global_assets.save_file).unwrap().levels.len()
        ))
        .state("Browsing menus")
        .assets(activity::Assets::default().large("icon".to_owned(), Some("GDClone".to_owned())))
        .button(activity::Button {
            label: "Get GDClone".to_owned(),
            url: "https://github.com/opstic/gdclone/releases".to_owned(),
        })
        .into();

    let main_container = commands
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                ..default()
            },
            background_color: Color::NONE.into(),
            ..default()
        })
        .id();
    commands.entity(main_container).insert(SelectMenu);

    let select_window = commands
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                size: Size::new(Val::Percent(70.0), Val::Percent(80.0)),
                ..default()
            },
            background_color: Color::rgb(0.15, 0.15, 0.15).into(),
            ..default()
        })
        .id();
    commands.entity(main_container).add_child(select_window);

    let title = commands
        .spawn(
            TextBundle::from_section(
                "Loaded Levels",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 50.,
                    color: Color::WHITE,
                },
            )
            .with_style(Style {
                margin: UiRect::new(Val::Auto, Val::Auto, Val::Auto, Val::Auto),
                ..default()
            }),
        )
        .id();
    commands.entity(select_window).add_child(title);

    let panel_frame = commands
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                align_self: AlignSelf::Stretch,
                size: Size::height(Val::Percent(90.)),
                overflow: Overflow::Hidden,
                ..default()
            },
            background_color: Color::rgb(0.10, 0.10, 0.10).into(),
            ..default()
        })
        .id();
    commands.entity(select_window).add_child(panel_frame);

    let panel = commands
        .spawn(NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Column,
                max_size: Size::UNDEFINED,
                align_items: AlignItems::Center,
                padding: UiRect::new(Val::Px(10.), Val::Px(10.), Val::Px(10.), Val::Undefined),
                ..default()
            },
            ..default()
        })
        .id();
    commands.entity(panel).insert(ScrollingList::default());
    commands.entity(panel_frame).add_child(panel);

    for (index, level) in saves
        .get(&global_assets.save_file)
        .unwrap()
        .levels
        .iter()
        .enumerate()
    {
        let level_entry = commands
            .spawn(NodeBundle {
                style: Style {
                    align_items: AlignItems::Center,
                    size: Size::new(Val::Percent(100.), Val::Px(125.)),
                    margin: UiRect::bottom(Val::Px(10.)),
                    padding: UiRect::all(Val::Px(15.)),
                    ..default()
                },
                background_color: Color::rgb(0.12, 0.12, 0.12).into(),
                ..default()
            })
            .id();
        commands.entity(panel).add_child(level_entry);

        let level_info_container = commands
            .spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    size: Size::new(Val::Percent(75.), Val::Percent(100.)),
                    align_items: AlignItems::Start,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                ..default()
            })
            .id();
        commands.entity(level_entry).add_child(level_info_container);

        let level_name = commands
            .spawn(TextBundle::from_section(
                &level.name,
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 50.,
                    color: Color::WHITE,
                },
            ))
            .id();
        commands.entity(level_info_container).add_child(level_name);

        let level_secondary_info_container = commands
            .spawn(NodeBundle {
                style: Style {
                    align_items: AlignItems::Center,
                    gap: Size::width(Val::Px(15.)),
                    ..default()
                },
                ..default()
            })
            .id();
        commands
            .entity(level_info_container)
            .add_child(level_secondary_info_container);

        let level_creator = commands
            .spawn(TextBundle::from_section(
                &level.creator,
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 35.,
                    color: Color::GRAY,
                },
            ))
            .id();
        commands
            .entity(level_secondary_info_container)
            .add_child(level_creator);

        if let Some(level_id) = level.id {
            let level_id = commands
                .spawn(TextBundle::from_section(
                    &(level_id.to_string()),
                    TextStyle {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 25.,
                        color: Color::DARK_GRAY,
                    },
                ))
                .id();
            commands
                .entity(level_secondary_info_container)
                .add_child(level_id);
        }

        let open_button = commands
            .spawn(ButtonBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    size: Size::new(Val::Px(75.), Val::Px(50.)),
                    margin: UiRect::left(Val::Auto),
                    ..default()
                },
                ..default()
            })
            .id();
        commands
            .entity(open_button)
            .insert(OpenButton { level_index: index });
        commands.entity(level_entry).add_child(open_button);

        let button_text = commands
            .spawn(TextBundle::from_section(
                "Open",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 25.0,
                    color: Color::rgb(0.8, 0.8, 0.8),
                },
            ))
            .id();
        commands.entity(open_button).add_child(button_text);
    }
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
    mut query_list: Query<(&mut ScrollingList, &mut Style, &Parent, &Node)>,
    query_node: Query<&Node>,
) {
    for mouse_wheel_event in mouse_wheel_events.iter() {
        for (mut scrolling_list, mut style, parent, list_node) in &mut query_list {
            let items_height = list_node.size().y;
            let container_height = query_node.get(parent.get()).unwrap().size().y;

            let max_scroll = (items_height - container_height).max(0.);

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
