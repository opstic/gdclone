use crate::{GDLevel, GameStates, LevelAssets};
use bevy::prelude::*;
use iyes_loopless::prelude::AppLooplessStateExt;

pub(crate) struct PlayStatePlugin;

impl Plugin for PlayStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_enter_system(GameStates::PlayState, play_setup);
    }
}

fn play_setup(
    mut commands: Commands,
    level_assets: Res<LevelAssets>,
    mut levels: ResMut<Assets<GDLevel>>,
) {
    if let Some(level) = levels.remove(level_assets.level.id) {
        for object in level.inner_level {
            commands.spawn_bundle(SpriteBundle {
                transform: Transform {
                    translation: Vec3::from((object.x, object.y, 0.)),
                    rotation: Quat::from_rotation_z(-object.rot.to_radians()),
                    scale: Vec3::new(object.scale, object.scale, 0.),
                },
                sprite: Sprite {
                    custom_size: Some(Vec2::new(30., 30.)),
                    ..default()
                },
                ..default()
            });
        }
    }
}
