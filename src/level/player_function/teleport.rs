use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, With, Without, World};

use crate::level::player::Player;
use crate::level::player_function::{GameplayObject, PlayerFunction};
use crate::level::transform::Transform2d;
use crate::level::trigger::Activated;

type TeleportSystemParam = (
    Query<'static, 'static, &'static mut Transform2d, (With<Player>, Without<GameplayObject>)>,
    Query<'static, 'static, &'static mut Transform2d, With<GameplayObject>>,
);

#[derive(Default)]
pub(crate) struct Teleport {
    pub(crate) distance: f32,
}

impl PlayerFunction for Teleport {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let system_state: &mut SystemState<TeleportSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (mut player_query, gameplay_object_query) = system_state.get_mut(world);

        let Ok(object_y) = gameplay_object_query
            .get(origin_entity)
            .map(|transform| transform.translation.y)
        else {
            return;
        };

        let Ok(mut player_transform) = player_query.get_mut(player_entity) else {
            return;
        };

        player_transform.translation.y = object_y + self.distance;
        world.entity_mut(origin_entity).insert(Activated);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<TeleportSystemParam>::new(world))
    }
}
