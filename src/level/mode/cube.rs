use std::any::Any;
use std::f32::consts::{FRAC_2_PI, FRAC_PI_2, PI, TAU};

use bevy::ecs::system::SystemState;
use bevy::input::ButtonInput;
use bevy::log::info;
use bevy::prelude::{Entity, MouseButton, Query, Res, World};
use bevy::time::Time;
use instant::Instant;

use crate::level::mode::GameMode;
use crate::level::player::Player;
use crate::level::transform::Transform2d;

type CubeSystemParam = (
    Res<'static, Time>,
    Res<'static, ButtonInput<MouseButton>>,
    Query<'static, 'static, (&'static mut Player, &'static mut Transform2d)>,
);

#[derive(Default)]
pub(crate) struct CubeMode {
    a: Option<Instant>,
}

const GRAVITY: f32 = 0.958199;
const JUMP_HEIGHT: f32 = 11.180032;
const VELOCITY_LIMIT: f32 = 15.;

impl GameMode for CubeMode {
    fn update(
        &mut self,
        world: &mut World,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<CubeSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (time, mouse_input, mut player_query) = system_state.get_mut(world);

        let (mut player, mut transform) = player_query.get_mut(player_entity).unwrap();

        if player.on_ground {
            if let Some(a) = self.a {
                info!("{:?}", a.elapsed());
                self.a = None;
            }

            let rotation = transform.angle % TAU;
            let mut target = (rotation * FRAC_2_PI).fract();

            if target.abs() > 0.5 {
                target = (1. - target.abs()).copysign(-target);
            }

            transform.angle -= (FRAC_PI_2 * target / 0.075) * time.delta_seconds();

            if mouse_input.pressed(MouseButton::Left) {
                self.a = Some(Instant::now());
                player.velocity.y = JUMP_HEIGHT;
                player.on_ground = false;
                return;
            }
        } else {
            transform.angle -= (PI / (1.3 / 3.)) * time.delta_seconds();
        }

        if player.velocity.y < -GRAVITY * 2. {
            player.on_ground = false;
        }

        player.velocity.y -= GRAVITY * 60. * 0.9 * time.delta_seconds();
        player.velocity.y = player.velocity.y.max(-VELOCITY_LIMIT);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<CubeSystemParam>::new(world))
    }
}
