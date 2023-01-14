pub(crate) mod color;
pub(crate) mod de;
pub(crate) mod easing;
// pub(crate) mod object;
pub(crate) mod trigger;

// use crate::level::object::{Object, StartObject};
use crate::level::trigger::TriggerSystems::TickTriggers;
use crate::level::trigger::{finish_triggers, tick_triggers, TriggerCompleted, TriggerSystems};
use bevy::app::{App, CoreStage, Plugin};
use bevy::prelude::{IntoSystemDescriptor, RunCriteriaDescriptorCoercion};
use serde::{Deserialize, Deserializer};

#[derive(Default)]
pub(crate) struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_to_stage(
            CoreStage::PostUpdate,
            tick_triggers
                .label(TriggerSystems::TickTriggers)
                .after(TriggerSystems::ActivateTriggers),
        )
        .add_system_to_stage(
            CoreStage::PostUpdate,
            finish_triggers
                .label(TriggerSystems::DeactivateTriggers)
                .after(TickTriggers),
        )
        .add_event::<TriggerCompleted>();
    }
}

// struct Level {
//     name: String,
//     description: Option<String>,
//     id: Option<u64>,
//     start_object: StartObject,
//     objects: Vec<Object>,
// }
