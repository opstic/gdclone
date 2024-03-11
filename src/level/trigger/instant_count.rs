use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, Res, ResMut, With, Without, World};
use float_next_after::NextAfter;

use crate::level::color::ObjectColorCalculated;
use crate::level::group::{GlobalGroup, GlobalGroups, ObjectGroups};
use crate::level::trigger::pickup::PickupValues;
use crate::level::trigger::{
    Activated, GlobalTriggers, MultiActivate, SpawnActivate, Trigger, TriggerData, TriggerFunction,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct InstantCountTrigger {
    pub(crate) item_id: u64,
    pub(crate) target_count: i64,
    pub(crate) mode: InstantCountMode,
    pub(crate) target_group: u64,
    pub(crate) activate: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum InstantCountMode {
    #[default]
    Equal,
    Larger,
    Smaller,
}

type InstantCountTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Res<'static, PickupValues>,
    Res<'static, GlobalTriggers>,
    ResMut<'static, TriggerData>,
    Query<'static, 'static, &'static mut GlobalGroup>,
    Query<
        'static,
        'static,
        (
            Entity,
            &'static Trigger,
            &'static ObjectGroups,
            &'static ObjectColorCalculated,
            Option<&'static MultiActivate>,
        ),
        (With<SpawnActivate>, Without<Activated>),
    >,
);

impl TriggerFunction for InstantCountTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        progress: f32,
        range: Range<f32>,
    ) {
        if progress != 1. {
            return;
        }

        let system_state: &mut SystemState<InstantCountTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let (
            global_groups,
            pickup_values,
            global_triggers,
            mut trigger_data,
            mut group_query,
            trigger_query,
        ) = system_state.get_mut(world);

        let Some(entry) = pickup_values.0.get(self.item_id as usize) else {
            return;
        };

        if !match self.mode {
            InstantCountMode::Equal => *entry == self.target_count,
            InstantCountMode::Larger => *entry > self.target_count,
            InstantCountMode::Smaller => *entry < self.target_count,
        } {
            return;
        }

        let Some(group_entity) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let Ok(mut group) = group_query.get_mut(*group_entity) else {
            return;
        };

        group.enabled = self.activate;

        if !group.enabled {
            return;
        }

        let trigger_time = global_triggers.speed_changes.time_for_pos(range.start);
        let start_time = trigger_time + self.duration();
        let start_pos = global_triggers.speed_changes.pos_for_time(start_time);

        let mut activated = Vec::new();

        for (entity, trigger, object_groups, calculated, multi_activate) in
            trigger_query.iter_many(&group.root_entities)
        {
            if !calculated.enabled {
                continue;
            }

            let mut end_pos = global_triggers
                .speed_changes
                .pos_for_time(start_time + trigger.0.duration());

            if end_pos == start_pos {
                end_pos = end_pos.next_after(f32::INFINITY);
            }

            trigger_data.to_spawn.push((
                entity,
                trigger.clone(),
                object_groups.groups.clone(),
                start_pos..end_pos,
            ));

            if multi_activate.is_some() {
                continue;
            }

            activated.push(entity);
        }

        for entity in activated {
            world.entity_mut(entity).insert(Activated);
        }
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<InstantCountTriggerSystemParam>::new(world))
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

    fn post(&self) -> bool {
        true
    }
}
