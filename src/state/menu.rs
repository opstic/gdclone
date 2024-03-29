use bevy::app::{App, Plugin, Update};
use bevy::asset::Handle;
use bevy::log::info;
use bevy::prelude::{
    in_state, Commands, Entity, IntoSystemConfigs, NextState, Query, ResMut, Resource, Window,
};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::utils::HashMap;
use bevy::window::{PresentMode, WindowMode};
use bevy_egui::EguiContexts;
use bevy_kira_audio::AudioSource;
use egui::{Button, Color32};
use futures_lite::future;

use crate::api::robtop::RobtopApi;
use crate::api::ServerApi;
use crate::level::{LevelInfo, SongInfo};
use crate::state::prepare::LevelToDownload;
use crate::state::GameState;

pub(crate) struct MenuStatePlugin;

impl Plugin for MenuStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelBrowserState>()
            .add_systems(Update, render_menu_gui.run_if(in_state(GameState::Menu)));
    }
}

#[derive(Resource)]
pub(crate) struct LevelBrowserState {
    search: String,
    response: Vec<LevelInfo>,
    task: Option<Task<Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error>>>,
    pub(crate) use_song: bool,
    pub(crate) song_infos: HashMap<u64, SongInfo>,
    pub(crate) stored_songs: HashMap<u64, Handle<AudioSource>>,
    pub(crate) low_detail: bool,
}

impl Default for LevelBrowserState {
    fn default() -> Self {
        Self {
            search: "".to_string(),
            response: Vec::new(),
            task: None,
            use_song: true,
            song_infos: HashMap::new(),
            stored_songs: HashMap::new(),
            low_detail: false,
        }
    }
}

fn render_menu_gui(
    mut commands: Commands,
    mut browser_state: ResMut<LevelBrowserState>,
    mut contexts: EguiContexts,
    mut state: ResMut<NextState<GameState>>,
    mut windows: Query<(Entity, &mut Window)>,
) {
    egui::Window::new("Level Browser")
        .vscroll(true)
        .show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Search: ");
                let response = ui.text_edit_singleline(&mut browser_state.search);
                if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || ui.button("Search").clicked()
                {
                    info!("Searching for {}", browser_state.search);
                    let query_string = browser_state.search.clone();
                    browser_state.task = Some(AsyncComputeTaskPool::get().spawn(async {
                        let api = RobtopApi::default();
                        api.search_levels(query_string).await
                    }));
                }

                ui.separator();
                ui.checkbox(&mut browser_state.use_song, "Use Song");
                ui.separator();
                ui.checkbox(&mut browser_state.low_detail, "Low Detail");
                ui.separator();

                let (entity, mut window) = windows.single_mut();

                egui::ComboBox::from_label("Window Mode")
                    .selected_text(format!("{:?}", window.mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut window.mode, WindowMode::Windowed, "Windowed");
                        ui.selectable_value(&mut window.mode, WindowMode::Fullscreen, "Fullscreen");
                        ui.selectable_value(
                            &mut window.mode,
                            WindowMode::BorderlessFullscreen,
                            "BorderlessFullscreen",
                        );
                    });
                ui.separator();
                let mut vsync = match window.present_mode {
                    PresentMode::AutoVsync => true,
                    PresentMode::AutoNoVsync => false,
                    _ => false,
                };
                ui.checkbox(&mut vsync, "VSync");
                window.present_mode = if vsync {
                    PresentMode::AutoVsync
                } else {
                    PresentMode::AutoNoVsync
                };
                ui.separator();
                if ui
                    .add(Button::new("Exit").fill(Color32::DARK_RED))
                    .clicked()
                {
                    commands.entity(entity).despawn();
                }
            });

            ui.separator();

            if let Some(task) = &mut browser_state.task {
                if let Some(task_result) = future::block_on(future::poll_once(task)) {
                    let (level_infos, song_infos) = task_result.unwrap();
                    browser_state.response = level_infos;
                    browser_state.song_infos.extend(song_infos);
                    browser_state.task = None;
                } else {
                    ui.label("Loading...");
                }
            } else if !browser_state.response.is_empty() {
                for level in &browser_state.response {
                    ui.horizontal(|ui| {
                        ui.label(&level.name);
                        if ui.button("Open").clicked() {
                            commands.insert_resource(LevelToDownload(level.clone()));
                            state.set(GameState::Prepare);
                        }
                    });
                }
            } else {
                ui.label("Nothing found :(");
            }
        });
}
