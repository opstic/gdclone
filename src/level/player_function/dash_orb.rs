use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, With, World};

use crate::level::player::Player;
use crate::level::player_function::{GameplayObject, PlayerFunction};
use crate::level::transform::Transform2d;
use crate::level::trigger::Activated;

type DashOrbSystemParam = (
    Query<'static, 'static, &'static mut Player>,
    Query<'static, 'static, &'static Transform2d, With<GameplayObject>>,
);

pub(crate) struct DashOrb {
    pub(crate) flip: bool,
}

impl PlayerFunction for DashOrb {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<DashOrbSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (mut player_query, gameplay_object_query) = system_state.get_mut(world);

        let Ok(mut player) = player_query.get_mut(player_entity) else {
            return;
        };

        if !player.buffered_input {
            return;
        }
        player.buffered_input = false;

        let Ok(orb_transform) = gameplay_object_query.get(origin_entity) else {
            return;
        };

        player.dash = Some(orb_transform.angle);

        if self.flip {
            player.flipped = !player.flipped;
        }

        player.on_ground = false;

        world.entity_mut(origin_entity).insert(Activated);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<DashOrbSystemParam>::new(world))
    }
}
