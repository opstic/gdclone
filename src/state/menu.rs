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

use crate::api::{DefaultApi, ServerApi};
use crate::level::trigger::process_triggers;
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
    #[cfg(not(target_arch = "wasm32"))]
    task: Option<Task<Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error>>>,
    #[cfg(target_arch = "wasm32")]
    result: Option<
        crossbeam_channel::Receiver<
            Result<(Vec<LevelInfo>, HashMap<u64, SongInfo>), anyhow::Error>,
        >,
    >,
    pub(crate) use_song: bool,
    pub(crate) song_infos: HashMap<u64, SongInfo>,
    pub(crate) stored_songs: HashMap<u64, Handle<AudioSource>>,
    pub(crate) low_detail: bool,
    pub(crate) start_paused: bool,
}

impl Default for LevelBrowserState {
    fn default() -> Self {
        Self {
            search: "".to_string(),
            response: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            task: None,
            #[cfg(target_arch = "wasm32")]
            result: None,
            use_song: true,
            song_infos: HashMap::new(),
            stored_songs: HashMap::new(),
            low_detail: false,
            start_paused: false,
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
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        browser_state.task = Some(AsyncComputeTaskPool::get().spawn(async {
                            let api = DefaultApi::default();
                            api.search_levels(query_string).await
                        }));
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        wasm_bindgen_futures::spawn_local(async move {
                            let api = DefaultApi::default();
                            let _ = tx.send(api.search_levels(query_string).await);
                        });
                        browser_state.result = Some(rx);
                    }
                }

                ui.separator();
                ui.checkbox(&mut browser_state.use_song, "Use Song");
                ui.separator();
                ui.checkbox(&mut browser_state.low_detail, "Low Detail");
                ui.separator();
                ui.checkbox(&mut browser_state.start_paused, "Start Paused");
                ui.separator();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let (entity, mut window) = windows.single_mut();
                    egui::ComboBox::from_label("Window Mode")
                        .selected_text(format!("{:?}", window.mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut window.mode, WindowMode::Windowed, "Windowed");
                            #[cfg(not(target_arch = "wasm32"))]
                            ui.selectable_value(
                                &mut window.mode,
                                WindowMode::Fullscreen,
                                "Fullscreen",
                            );
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
                }

                #[cfg(target_arch = "wasm32")]
                ui.label("Use F11 for fullscreen toggle");
            });

            ui.separator();

            #[cfg(not(target_arch = "wasm32"))]
            {
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
            }

            #[cfg(target_arch = "wasm32")]
            {
                if let Some(result) = &mut browser_state.result {
                    let Ok(search) = result.try_recv() else {
                        ui.label("Loading...");
                        return;
                    };
                    let (level_infos, song_infos) = search.unwrap();
                    browser_state.response = level_infos;
                    browser_state.song_infos.extend(song_infos);
                    browser_state.result = None;
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
            }
        });
}
