use std::any::Any;

use bevy::prelude::{Entity, World};

use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct EmptyTrigger;

impl TriggerFunction for EmptyTrigger {
    fn execute(
        &self,
        _: &mut World,
        _: Entity,
        _: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        _: f32,
    ) {
    }

    fn create_system_state(&self, _: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(())
    }

    fn target_id(&self) -> u64 {
        0
    }

    fn duration(&self) -> f32 {
        0.
    }

    fn exclusive(&self) -> bool {
        false
    }
}
