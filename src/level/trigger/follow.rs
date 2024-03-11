use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{Entity, Query, Res, World};

use crate::level::group::{GlobalGroup, GlobalGroupDeltas, GlobalGroups, RotationKind};
use crate::level::transform::Transform2d;
use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct FollowTrigger {
    pub(crate) duration: f32,
    pub(crate) target_group: u64,
    pub(crate) follow_group: u64,
    pub(crate) scale: Vec2,
}

type FollowTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Query<'static, 'static, (&'static GlobalGroup, &'static mut GlobalGroupDeltas)>,
    Query<'static, 'static, &'static Transform2d>,
);

impl TriggerFunction for FollowTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        _: f32,
        _: Range<f32>,
    ) {
        let system_state: &mut SystemState<FollowTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (global_groups, mut group_delta_query, transform_query) = system_state.get_mut(world);

        let Some(target_group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let Some(follow_group_entity) = global_groups.0.get(self.follow_group as usize) else {
            return;
        };

        let Ok((follow_group, follow_group_delta)) = group_delta_query.get(*follow_group_entity)
        else {
            return;
        };

        if follow_group.root_entities.len() != 1 {
            return;
        }

        let following_entity = follow_group.root_entities[0];

        let mut delta = follow_group_delta.translation_delta;

        if let RotationKind::Around(center_entity, rotation, _) = follow_group_delta.rotation {
            let Ok(followed_transform) = transform_query.get(following_entity) else {
                return;
            };

            let Ok(center_transform) = transform_query.get(center_entity) else {
                return;
            };

            let cos_sin = Vec2::from_angle(rotation);

            let mut rotated_transform = *followed_transform;
            rotated_transform.translate_around_cos_sin(center_transform.translation.xy(), cos_sin);

            delta += rotated_transform.translation.xy() - followed_transform.translation.xy();
        }

        let Ok((_, mut target_group_delta)) = group_delta_query.get_mut(*target_group_entity)
        else {
            return;
        };

        target_group_delta.translation_delta += delta * self.scale;
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<FollowTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        self.target_group
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }

    fn post(&self) -> bool {
        true
    }
}
