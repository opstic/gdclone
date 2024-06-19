use std::any::Any;
use std::f32::consts::{FRAC_PI_2, PI};

use bevy::ecs::system::SystemState;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::prelude::{Entity, MouseButton, Query, Res, World};
use bevy::time::Time;

use crate::level::player::{Player, JUMP_HEIGHT};
use crate::level::player_function::PlayerFunction;
use crate::level::transform::Transform2d;
use crate::utils::lerp;

type CubeSystemParam = (
    Res<'static, Time>,
    Res<'static, ButtonInput<MouseButton>>,
    Query<'static, 'static, (&'static mut Player, &'static mut Transform2d)>,
);

#[derive(Default)]
pub(crate) struct CubeMode {
    rotation_target: Option<(f32, f32)>,
    rotation_progress: f32,
}

const GRAVITY: f64 = 0.958199;
const VELOCITY_LIMIT: f64 = 15.;

impl PlayerFunction for CubeMode {
    fn update(
        &mut self,
        world: &mut World,
        _: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<CubeSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (time, mouse_input, mut player_query) = system_state.get_mut(world);

        let (mut player, mut transform) = player_query.get_mut(player_entity).unwrap();

        if !player.mini {
            transform.scale = Vec2::splat(1.);
        } else {
            transform.scale = Vec2::splat(0.6);
        }

        if player.on_ground {
            let (initial_rotation, target_rotation) =
                self.rotation_target.get_or_insert_with(|| {
                    let target = (transform.angle / FRAC_PI_2).round() * FRAC_PI_2;

                    (transform.angle, target)
                });

            self.rotation_progress += time.delta_seconds() / 0.075;

            transform.angle = lerp(
                *initial_rotation,
                *target_rotation,
                self.rotation_progress.min(1.),
            );

            if mouse_input.pressed(MouseButton::Left) {
                player.buffered_input = false;
                player.velocity.y = JUMP_HEIGHT * if !player.mini { 1. } else { 0.8 };
                player.on_ground = false;
                return;
            }
        } else {
            self.rotation_target = None;
            self.rotation_progress = 0.;

            let flip_factor = if !player.flipped { 1. } else { -1. };
            let mini_factor = if !player.mini { 1. } else { 0.8 };
            transform.angle -= (PI * flip_factor / (0.41 * mini_factor)) * time.delta_seconds();
        }

        if player.velocity.y < -GRAVITY * 2. {
            player.on_ground = false;
        }

        player.velocity.y -= GRAVITY * 60. * 0.9 * time.delta_seconds_f64();
        player.velocity.y = player.velocity.y.max(-VELOCITY_LIMIT);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<CubeSystemParam>::new(world))
    }
}
