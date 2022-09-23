use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

mod loading;
mod play;

use loading::LoadingStatePlugin;
use play::PlayStatePlugin;

#[derive(Component, Clone, Eq, PartialEq, Debug, Hash, Copy)]
pub(crate) enum GameStates {
    LoadingState,
    PlayState,
}

pub(crate) struct StatePlugins;

impl PluginGroup for StatePlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(LoadingStatePlugin).add(PlayStatePlugin);
    }
}
