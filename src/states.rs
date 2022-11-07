use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

mod level_select;
mod loading;
mod play;

use level_select::LevelSelectStatePlugin;
use loading::LoadingStatePlugin;
use play::PlayStatePlugin;

#[derive(Component, Clone, Eq, PartialEq, Debug, Hash, Copy)]
pub(crate) enum GameState {
    Loading,
    LevelSelect,
    Play,
}

pub(crate) struct StatePlugins;

impl PluginGroup for StatePlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group
            .add(LoadingStatePlugin)
            .add(LevelSelectStatePlugin)
            .add(PlayStatePlugin);
    }
}
