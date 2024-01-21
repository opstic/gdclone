use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::math::{BVec2, Vec2};
use bevy::prelude::{Query, Res, Transform, Without, World};

use crate::level::easing::Easing;
use crate::level::group::{GlobalGroupDeltas, GlobalGroups};
use crate::level::object::Object;
use crate::level::player::Player;
use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct MoveTrigger {
    pub(crate) duration: f32,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) offset: Vec2,
    pub(crate) lock: BVec2,
}

type MoveTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Query<'static, 'static, &'static mut GlobalGroupDeltas>,
    Query<'static, 'static, (&'static Player, &'static Transform), Without<Object>>,
);

impl TriggerFunction for MoveTrigger {
    fn execute(
        &self,
        world: &mut World,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
    ) {
        let system_state: &mut SystemState<MoveTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (global_groups, mut group_delta_query, player_query) = system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let Ok(mut global_group_delta) = group_delta_query.get_mut(*group_entity) else {
            return;
        };

        let amount = self.easing.sample(progress) - self.easing.sample(previous_progress);

        let mut delta = self.offset * amount;

        if self.lock.any() {
            let (player, transform) = player_query.single();

            if self.lock.x {
                delta.x += transform.translation.x - player.last_translation.x;
            }

            if self.lock.y {
                delta.y += transform.translation.y - player.last_translation.y;
            }
        }

        global_group_delta.translation_delta += delta;
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<MoveTriggerSystemParam>::new(world))
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }
}
