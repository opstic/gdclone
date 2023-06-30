use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use level_select::LevelSelectStatePlugin;
use loading::LoadingStatePlugin;
use play::PlayStatePlugin;

pub(crate) mod level_select;
pub(crate) mod loading;
pub(crate) mod play;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub(crate) enum GameState {
    #[default]
    Loading,
    LevelSelect,
    Play,
}

pub(crate) struct StatePlugins;

impl PluginGroup for StatePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(LoadingStatePlugin)
            .add(LevelSelectStatePlugin)
            .add(PlayStatePlugin)
    }
}
