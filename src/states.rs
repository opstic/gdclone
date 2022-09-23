use bevy::prelude::*;

mod loading;
mod play;

#[derive(Component, Clone, Eq, PartialEq, Debug, Hash, Copy)]
pub(crate) enum GameStates {
    LoadingState,
    PlayState,
}
