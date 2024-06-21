use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::prelude::{Entity, MouseButton, Query, Res, World};

use crate::level::player::Player;
use crate::level::player_function::PlayerFunction;
use crate::level::transform::Transform2d;

type WaveSystemParam = (
    Res<'static, ButtonInput<MouseButton>>,
    Query<'static, 'static, (&'static mut Player, &'static mut Transform2d)>,
);

pub(crate) struct WaveMode;

impl PlayerFunction for WaveMode {
    fn update(
        &mut self,
        world: &mut World,
        _: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<WaveSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (mouse_input, mut player_query) = system_state.get_mut(world);

        let pressed = mouse_input.pressed(MouseButton::Left);

        let (mut player, mut transform) = player_query.get_mut(player_entity).unwrap();

        player.buffered_input = false;

        if !player.mini {
            player.snap_distance = (1., 6.);
            transform.scale = Vec2::splat(2. / 3.);
        } else {
            player.snap_distance = (-3., 6.);
            transform.scale = Vec2::splat((2. / 3.) * 0.6);
        }

        if pressed {
            player.on_ground = false;
        }

        let mut angle: f32 = if !player.mini { 45. } else { 22.5 } * if pressed { 1. } else { -1. };
        angle = angle.to_radians();

        let x_delta = player.velocity.x * player.speed;

        player.velocity.y = (x_delta / angle.tan() as f64) / (60. * 0.9);

        if !pressed && player.on_ground {
            transform.angle = 0.;
        } else {
            transform.angle = angle;
        }
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<WaveSystemParam>::new(world))
    }
}
