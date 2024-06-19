use std::any::Any;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, With, World};

use crate::level::player::Player;
use crate::level::player_function::{GameplayObject, PlayerFunction};
use crate::level::transform::Transform2d;
use crate::level::trigger::Activated;

type GravityPadSystemParam = (
    Query<'static, 'static, &'static mut Player>,
    Query<'static, 'static, &'static Transform2d, With<GameplayObject>>,
);

pub(crate) struct GravityPad;

impl PlayerFunction for GravityPad {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<GravityPadSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (mut player_query, gameplay_object_query) = system_state.get_mut(world);

        let Ok(mut player) = player_query.get_mut(player_entity) else {
            return;
        };

        let Ok(gravity_pad_transform) = gameplay_object_query.get(origin_entity) else {
            return;
        };

        let rotation_sign = if gravity_pad_transform.scale.y.is_sign_negative() {
            PI
        } else {
            0.
        };

        let normalized_angle = (gravity_pad_transform.angle + rotation_sign).rem_euclid(TAU);

        let target_flipped = !(normalized_angle > FRAC_PI_2 && normalized_angle < (FRAC_PI_2 * 3.));

        if player.flipped == target_flipped {
            return;
        }

        if player.pad_activated_frame {
            return;
        }
        player.flipped = target_flipped;
        player.velocity.y = -0.8 * 16. * 0.5;

        if player.mini {
            player.velocity.y *= 0.8;
        }

        player.on_ground = false;
        player.pad_activated_frame = true;

        world.entity_mut(origin_entity).insert(Activated);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<GravityPadSystemParam>::new(world))
    }
}
