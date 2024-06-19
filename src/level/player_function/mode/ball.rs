use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::math::Vec2;
use bevy::prelude::{Entity, Query, Res, World};
use bevy::time::Time;

use crate::level::player::{Player, JUMP_HEIGHT};
use crate::level::player_function::PlayerFunction;
use crate::level::transform::Transform2d;

type BallSystemParam = (
    Res<'static, Time>,
    Query<'static, 'static, (&'static mut Player, &'static mut Transform2d)>,
);

pub(crate) struct BallMode;

const GRAVITY: f64 = 0.958199;
const VELOCITY_LIMIT: f64 = 15.;

impl PlayerFunction for BallMode {
    fn update(
        &mut self,
        world: &mut World,
        _: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<BallSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (time, mut player_query) = system_state.get_mut(world);

        let (mut player, mut transform) = player_query.get_mut(player_entity).unwrap();

        if !player.mini {
            transform.scale = Vec2::splat(1.);
        } else {
            transform.scale = Vec2::splat(0.6);
        }

        transform.angle = 0.;

        if player.pad_activated_frame {
            player.velocity.y *= 0.6;
        }

        if player.orb_activated_frame {
            player.velocity.y *= 0.7;
        }

        if player.on_ground && player.buffered_input {
            player.buffered_input = false;
            player.velocity.y = -JUMP_HEIGHT * if !player.mini { 1. } else { 0.8 } * 0.5 * 0.6;
            player.flipped = !player.flipped;
            player.on_ground = false;
            return;
        }

        if player.velocity.y < -GRAVITY * 2. {
            player.on_ground = false;
        }

        player.velocity.y -= GRAVITY * (60. * 0.9 * time.delta_seconds_f64()) * 0.6;
        player.velocity.y = player.velocity.y.max(-VELOCITY_LIMIT);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<BallSystemParam>::new(world))
    }
}
