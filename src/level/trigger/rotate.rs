use std::any::Any;
use std::f32::consts::TAU;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, Res, World};

use crate::level::easing::Easing;
use crate::level::group::{GlobalGroup, GlobalGroupDeltas, GlobalGroups, RotationKind};
use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct RotateTrigger {
    pub(crate) duration: f32,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) center_group: u64,
    pub(crate) degrees: f32,
    pub(crate) times360: f32,
    pub(crate) lock_rotation: bool,
}

type RotateTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Query<'static, 'static, &'static GlobalGroup>,
    Query<'static, 'static, &'static mut GlobalGroupDeltas>,
);

impl TriggerFunction for RotateTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
        _: Range<f32>,
    ) {
        let system_state: &mut SystemState<RotateTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (global_groups, group_query, mut group_delta_query) = system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let center = global_groups
            .0
            .get(self.center_group as usize)
            .and_then(|entity| group_query.get(*entity).ok())
            .and_then(|group| {
                if group.root_entities.len() == 1 {
                    Some(group.root_entities[0])
                } else {
                    None
                }
            });

        let Ok(mut global_group_delta) = group_delta_query.get_mut(*group_entity) else {
            return;
        };

        let amount = self.easing.sample(progress) - self.easing.sample(previous_progress);

        let delta = (TAU * self.times360 + self.degrees) * amount;

        if let Some(center) = center {
            global_group_delta.rotation = RotationKind::Around(center, delta, self.lock_rotation);
        } else {
            global_group_delta.rotation = RotationKind::Angle(delta);
        }
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<RotateTriggerSystemParam>::new(world))
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
        false
    }
}
