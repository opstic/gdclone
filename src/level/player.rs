use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{Component, Query, Res, Transform};
use bevy::time::Time;

use crate::level::trigger::{GlobalTriggers, SpeedChange};

#[derive(Component)]
pub(crate) struct Player {
    pub(crate) last_translation: Vec2,
    pub(crate) velocity: Vec2,
    pub(crate) vertical_is_x: bool,
    pub(crate) reverse: bool,
    pub(crate) speed: f32,
    pub(crate) gravity: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            last_translation: Vec2::ZERO,
            // velocity: Vec2::new(0., 0.),
            velocity: Vec2::new(5.77 * 60., 0.),
            vertical_is_x: false,
            reverse: false,
            speed: 0.9,
            gravity: 0.,
        }
    }
}

// pub(crate) fn update_player_velocity(mut players: Query<&mut Player>) {
//     for mut player in &mut players {}
// }

pub(crate) fn update_player_pos(
    mut players: Query<(&mut Player, &mut Transform)>,
    speed_changes: Query<&SpeedChange>,
    time: Res<Time>,
    triggers: Res<GlobalTriggers>,
) {
    for (mut player, mut transform) in &mut players {
        let (_, speed_data) = triggers
            .speed_changes
            .speed_data_at_pos(transform.translation.x);
        let speed_change = speed_changes.get(speed_data.entity).unwrap();
        player.velocity.x = speed_change.forward_velocity;
        player.speed = speed_change.speed;

        if player.velocity == Vec2::ZERO {
            continue;
        }

        player.last_translation = transform.translation.xy();

        let slowed_delta = time.delta_seconds() * 0.9;

        transform.translation.x += player.velocity.x * time.delta_seconds() * player.speed;
        transform.translation.y += player.velocity.y * slowed_delta;
    }
}
