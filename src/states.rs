use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

mod loading;
mod play;
mod level_select;

use loading::LoadingStatePlugin;
use play::PlayStatePlugin;
use crate::GameStates::LevelSelectState;
use crate::states::level_select::LevelSelectStatePlugin;

#[derive(Component, Clone, Eq, PartialEq, Debug, Hash, Copy)]
pub(crate) enum GameStates {
    LoadingState,
    LevelSelectState,
    PlayState
}

pub(crate) struct StatePlugins;

impl PluginGroup for StatePlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(LoadingStatePlugin).add(LevelSelectStatePlugin).add(PlayStatePlugin);
    }
}
