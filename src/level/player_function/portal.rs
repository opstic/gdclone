use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, World};

use crate::level::player::Player;
use crate::level::player_function::PlayerFunction;
use crate::level::trigger::Activated;

type GravityPortalSystemParam = Query<'static, 'static, &'static mut Player>;

pub(crate) struct Portal {
    pub(crate) func: fn(&mut Player) -> bool,
}

impl PlayerFunction for Portal {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<GravityPortalSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let mut player_query = system_state.get_mut(world);

        let Ok(mut player) = player_query.get_mut(player_entity) else {
            return;
        };

        if !(self.func)(&mut player) {
            return;
        }

        world.entity_mut(origin_entity).insert(Activated);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<GravityPortalSystemParam>::new(world))
    }
}
