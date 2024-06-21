use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::prelude::{Entity, MouseButton, Query, Res, World};
use bevy::time::Time;

use crate::level::player::Player;
use crate::level::player_function::PlayerFunction;
use crate::level::transform::Transform2d;

type ShipSystemParam = (
    Res<'static, Time>,
    Res<'static, ButtonInput<MouseButton>>,
    Query<'static, 'static, (&'static mut Player, &'static mut Transform2d)>,
);

pub(crate) struct ShipMode;

const GRAVITY: f64 = 0.958199;

impl PlayerFunction for ShipMode {
    fn update(
        &mut self,
        world: &mut World,
        _: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<ShipSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (time, mouse_input, mut player_query) = system_state.get_mut(world);

        let pressed = mouse_input.pressed(MouseButton::Left);

        let (mut player, mut transform) = player_query.get_mut(player_entity).unwrap();

        player.buffered_input = false;

        if !player.mini {
            transform.scale = Vec2::splat(1.);
        } else {
            transform.scale = Vec2::splat(0.6);
        }

        transform.angle = 0.;

        let mini_factor = if !player.mini { 1. } else { 0.85 };

        let velocity_limit = (-6.4 / mini_factor)..(8.0 / mini_factor);

        let ship_accel = if pressed {
            -1.0
        } else if !pressed && !player.falling() {
            1.2
        } else {
            0.8
        };

        let boost_factor = if pressed && player.falling() {
            0.5
        } else {
            0.4
        };

        player.velocity.y -=
            GRAVITY * (60. * 0.9 * time.delta_seconds_f64()) * ship_accel * boost_factor
                / mini_factor;
        player.velocity.y = player
            .velocity
            .y
            .clamp(velocity_limit.start, velocity_limit.end);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<ShipSystemParam>::new(world))
    }
}
