use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, ResMut, Resource, World};

use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct PickupTrigger {
    pub(crate) item_id: u64,
    pub(crate) count: i64,
}

#[derive(Resource)]
pub(crate) struct PickupValues(pub(crate) [i64; 1000]);

impl Default for PickupValues {
    fn default() -> Self {
        Self([0; 1000])
    }
}

type PickupTriggerSystemParam = ResMut<'static, PickupValues>;

impl TriggerFunction for PickupTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        progress: f32,
        _: Range<f32>,
    ) {
        if progress != 1. {
            return;
        }

        let system_state: &mut SystemState<PickupTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let mut pickup_values = system_state.get_mut(world);

        let Some(entry) = pickup_values.0.get_mut(self.item_id as usize) else {
            return;
        };

        *entry += self.count;
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<PickupTriggerSystemParam>::new(world))
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

    fn post(&self) -> bool {
        false
    }
}
