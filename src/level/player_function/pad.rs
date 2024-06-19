use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, World};

use crate::level::player::Player;
use crate::level::player_function::PlayerFunction;
use crate::level::trigger::Activated;

type PadSystemParam = Query<'static, 'static, &'static mut Player>;

pub(crate) struct Pad {
    pub(crate) func: fn(&mut Player) -> bool,
}

impl PlayerFunction for Pad {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<PadSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let mut player_query = system_state.get_mut(world);

        let Ok(mut player) = player_query.get_mut(player_entity) else {
            return;
        };

        if !(self.func)(&mut player) {
            return;
        }

        if player.mini {
            player.velocity.y *= 0.8;
        }

        player.on_ground = false;
        player.pad_activated_frame = true;

        world.entity_mut(origin_entity).insert(Activated);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<PadSystemParam>::new(world))
    }
}
