use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::prelude::{Entity, MouseButton, Query, Res, World};
use bevy::time::Time;

use crate::level::player::{Player, JUMP_HEIGHT};
use crate::level::player_function::PlayerFunction;
use crate::level::transform::Transform2d;

type RobotSystemParam = (
    Res<'static, Time>,
    Res<'static, ButtonInput<MouseButton>>,
    Query<'static, 'static, (&'static mut Player, &'static mut Transform2d)>,
);

#[derive(Default)]
pub(crate) struct RobotMode {
    hold_timer: Option<f32>,
}

const GRAVITY: f64 = 0.958199 * 0.9;
const VELOCITY_LIMIT: f64 = 15.;

impl PlayerFunction for RobotMode {
    fn update(
        &mut self,
        world: &mut World,
        _: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<RobotSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (time, mouse_input, mut player_query) = system_state.get_mut(world);

        let (mut player, mut transform) = player_query.get_mut(player_entity).unwrap();

        transform.angle = 0.;

        if !player.mini {
            player.snap_distance = (5., 9.);
            transform.scale = Vec2::splat(1.);
        } else {
            player.snap_distance = (-1., 3.);
            transform.scale = Vec2::splat(0.6);
        }

        if player.on_ground && player.buffered_input {
            self.hold_timer = Some(0.);
            player.buffered_input = false;
            player.velocity.y = JUMP_HEIGHT * if !player.mini { 1. } else { 0.8 } * 0.5;
            player.on_ground = false;
            return;
        }

        if player.velocity.y < -GRAVITY * 2. {
            player.on_ground = false;
        }

        if !mouse_input.pressed(MouseButton::Left) {
            self.hold_timer = None;
        }

        match &mut self.hold_timer {
            Some(hold_timer) if *hold_timer < 1.5 => {
                *hold_timer += (60. * 0.9 * time.delta_seconds()) * 0.1;
            }
            _ => {
                player.velocity.y -= GRAVITY * (60. * 0.9 * time.delta_seconds_f64());
            }
        }

        player.velocity.y = player.velocity.y.max(-VELOCITY_LIMIT);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<RobotSystemParam>::new(world))
    }
}
