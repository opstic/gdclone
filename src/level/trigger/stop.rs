use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, ResMut, World};

use crate::level::trigger::{TriggerData, TriggerFunction};

#[derive(Clone, Debug, Default)]
pub(crate) struct StopTrigger {
    pub(crate) target_group: u64,
}

type StopTriggerSystemParam = ResMut<'static, TriggerData>;

impl TriggerFunction for StopTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        trigger_index: u32,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        _: f32,
        _: Range<f32>,
    ) {
        let system_state: &mut SystemState<StopTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let mut trigger_data = system_state.get_mut(world);
        trigger_data
            .stopped
            .insert(self.target_group, trigger_index);
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<StopTriggerSystemParam>::new(world))
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
