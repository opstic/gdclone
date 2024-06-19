use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, World};

use crate::level::player::Player;
use crate::level::player_function::PlayerFunction;
use crate::level::trigger::Activated;

type GravityOrbSystemParam = Query<'static, 'static, &'static mut Player>;
pub(crate) struct Orb {
    pub(crate) compute_force: fn(&mut Player) -> f64,
}

impl PlayerFunction for Orb {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<GravityOrbSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let mut player_query = system_state.get_mut(world);

        let Ok(mut player) = player_query.get_mut(player_entity) else {
            return;
        };

        if player.orb_activated_frame {
            return;
        }

        if !player.buffered_input {
            return;
        }
        player.buffered_input = false;

        player.velocity.y = (self.compute_force)(&mut player);

        if player.mini {
            player.velocity.y *= 0.8;
        }

        player.on_ground = false;
        player.orb_activated_frame = true;

        world.entity_mut(origin_entity).insert(Activated);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<GravityOrbSystemParam>::new(world))
    }
}
