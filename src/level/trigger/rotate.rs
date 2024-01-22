use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Query, Res, World};

use crate::level::easing::Easing;
use crate::level::group::{GlobalGroup, GlobalGroupDeltas, GlobalGroups};
use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct RotateTrigger {
    pub(crate) duration: f32,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) center_group: u64,
    pub(crate) degrees: i32,
    pub(crate) times360: i32,
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
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
    ) {
        let system_state: &mut SystemState<RotateTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (global_groups, group_query, mut group_delta_query) = system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        // This is horrendously bad
        let center =
            if let Some(center_group_entity) = global_groups.0.get(self.center_group as usize) {
                if let Ok(center_group) = group_query.get(*center_group_entity) {
                    if center_group.root_entities.len() == 1 {
                        Some(center_group.root_entities[0])
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

        let Ok(mut global_group_delta) = group_delta_query.get_mut(*group_entity) else {
            return;
        };

        let amount = self.easing.sample(progress) - self.easing.sample(previous_progress);

        let delta = -((360 * self.times360 + self.degrees) as f32 * amount).to_radians();

        if let Some(center) = center {
            global_group_delta.rotate_around = Some((center, delta, self.lock_rotation));
        } else {
            global_group_delta.rotation += delta;
        }
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<RotateTriggerSystemParam>::new(world))
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }
}
