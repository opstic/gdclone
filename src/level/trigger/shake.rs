use std::any::Any;
use std::f32::consts::TAU;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, ResMut, Resource, World};

use crate::level::trigger::TriggerFunction;

#[derive(Default, Resource)]
pub(crate) struct ShakeData(pub(crate) f32, pub(crate) f32);

#[derive(Clone, Debug, Default)]
pub(crate) struct ShakeTrigger {
    pub(crate) duration: f32,
    pub(crate) strength: f32,
    pub(crate) interval: f32,
}

type ShakeTriggerSystemParam = ResMut<'static, ShakeData>;

impl TriggerFunction for ShakeTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
        _: Range<f32>,
    ) {
        let system_state: &mut SystemState<ShakeTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let mut shake_data = system_state.get_mut(world);

        if progress == 1. {
            shake_data.0 = 0.;
            shake_data.1 = 0.;
            return;
        }

        let percent_interval = self.interval / self.duration;

        let rem_1 = previous_progress % percent_interval;
        let rem_2 = progress % percent_interval;

        if (rem_2 - rem_1) >= 0. && self.interval != 0. {
            return;
        }

        shake_data.0 = self.strength * 1.5 * fastrand::f32();
        shake_data.1 = TAU * fastrand::f32();
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<ShakeTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        0
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
