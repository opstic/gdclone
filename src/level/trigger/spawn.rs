use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Entity, Query, Res, ResMut, With, Without, World};

use crate::level::color::ObjectColorCalculated;
use crate::level::group::{GlobalGroup, GlobalGroups, ObjectGroups};
use crate::level::trigger::{
    Activated, GlobalTriggers, MultiActivate, SpawnActivate, Trigger, TriggerData, TriggerFunction,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct SpawnTrigger {
    pub(crate) target_group: u64,
    pub(crate) delay: f32,
}

type SpawnTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Res<'static, GlobalTriggers>,
    ResMut<'static, TriggerData>,
    Query<'static, 'static, &'static GlobalGroup>,
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

impl TriggerFunction for SpawnTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        trigger_index: u32,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        progress: f32,
        range: Range<f32>,
    ) {
        if progress != 1. {
            return;
        }

        let system_state: &mut SystemState<SpawnTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (global_groups, global_triggers, mut trigger_data, group_query, trigger_query) =
            system_state.get_mut(world);

        let trigger_time = global_triggers.speed_changes.time_for_pos(range.start);
        let start_time = trigger_time + self.duration();
        let start_pos = global_triggers.speed_changes.pos_for_time(start_time);

        let Some(target_group) = global_groups.0.get(self.target_group as usize) else {
            return;
        };

        let Ok(group) = group_query.get(*target_group) else {
            return;
        };

        let mut activated = Vec::new();

        for (entity, trigger, object_groups, calculated, multi_activate) in
            trigger_query.iter_many(&group.root_entities)
        {
            if !calculated.enabled {
                return;
            }

            let end_pos = global_triggers
                .speed_changes
                .pos_for_time(start_time + trigger.0.duration());

            trigger_data.to_spawn.push((
                entity,
                trigger.clone(),
                object_groups.groups.clone(),
                trigger_index,
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
        Box::new(SystemState::<SpawnTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        0
    }

    fn duration(&self) -> f32 {
        self.delay
    }

    fn exclusive(&self) -> bool {
        false
    }
}
