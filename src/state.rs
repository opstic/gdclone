use bevy::app::{App, Plugin};
use bevy::prelude::States;

use crate::state::level::LevelStatePlugin;
use crate::state::menu::MenuStatePlugin;
use crate::state::prepare::PrepareStatePlugin;
use crate::state::startup::StartupStatePlugin;

pub(crate) mod level;
mod menu;
mod prepare;
mod startup;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum GameState {
    #[default]
    Startup,
    Menu,
    Prepare,
    Level,
}

pub(crate) struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>().add_plugins((
            StartupStatePlugin,
            MenuStatePlugin,
            PrepareStatePlugin,
            LevelStatePlugin,
        ));
    }
}
