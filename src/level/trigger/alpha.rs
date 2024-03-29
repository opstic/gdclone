use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, Res, World};

use crate::level::group::{GlobalGroup, GlobalGroups};
use crate::level::trigger::TriggerFunction;
use crate::utils::{lerp, lerp_start};

#[derive(Clone, Debug, Default)]
pub(crate) struct AlphaTrigger {
    pub(crate) duration: f32,
    pub(crate) target_group: u64,
    pub(crate) target_opacity: f32,
}

type AlphaTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Query<'static, 'static, &'static mut GlobalGroup>,
);

impl TriggerFunction for AlphaTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
        _: Range<f32>,
    ) {
        let system_state: &mut SystemState<AlphaTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let (global_groups, mut group_query) = system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let Ok(mut global_group) = group_query.get_mut(*group_entity) else {
            return;
        };

        let original_opacity =
            lerp_start(global_group.opacity, self.target_opacity, previous_progress).clamp(0., 1.);
        global_group.opacity = lerp(original_opacity, self.target_opacity, progress).clamp(0., 1.);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<AlphaTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        self.target_group
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        true
    }

    fn post(&self) -> bool {
        false
    }
}
