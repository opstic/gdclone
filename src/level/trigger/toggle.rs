use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, Res, World};

use crate::level::group::{GlobalGroup, GlobalGroups};
use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct ToggleTrigger {
    pub(crate) target_group: u64,
    pub(crate) activate: bool,
}

type ToggleTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Query<'static, 'static, &'static mut GlobalGroup>,
);

impl TriggerFunction for ToggleTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        _: f32,
    ) {
        let system_state: &mut SystemState<ToggleTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let (global_groups, mut group_query) = system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let Ok(mut global_group) = group_query.get_mut(*group_entity) else {
            return;
        };

        global_group.enabled = self.activate;
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<ToggleTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        self.target_group
    }

    fn duration(&self) -> f32 {
        0.
    }

    fn exclusive(&self) -> bool {
        false
    }
}
