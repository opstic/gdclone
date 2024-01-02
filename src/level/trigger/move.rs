use bevy::ecs::system::SystemState;
use bevy::math::{BVec2, Vec2};
use bevy::prelude::{Query, Res, Transform, With, Without, World};

use crate::level::easing::Easing;
use crate::level::group::{GlobalGroupDeltas, GlobalGroups, TransformDelta};
use crate::level::object::Object;
use crate::level::player::Player;
use crate::level::trigger::{Trigger, TriggerFunction};

#[derive(Clone, Debug, Default)]
pub(crate) struct MoveTrigger {
    pub(crate) duration: f32,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) offset: Vec2,
    pub(crate) lock: BVec2,
}

impl TriggerFunction for MoveTrigger {
    fn execute(&self, world: &mut World, previous_progress: f32, progress: f32) {
        let mut system_state: SystemState<(
            Res<GlobalGroups>,
            Query<&mut GlobalGroupDeltas>,
            Query<&Transform, (With<Object>, Without<Trigger>)>,
            Query<(&Player, &Transform), Without<Object>>,
        )> = SystemState::new(world);

        let (global_groups, mut group_delta_query, object_transform_query, player_query) =
            system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(&self.target_group) else {
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

        global_group_delta
            .deltas
            .push(TransformDelta::Translate { delta });
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }
}
