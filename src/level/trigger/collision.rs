use std::any::Any;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Component, Entity, Query, Res, ResMut, With, Without, World};
use bevy::time::Time;
use float_next_after::NextAfter;

use crate::level::collision::ActiveCollider;
use crate::level::color::ObjectColorCalculated;
use crate::level::group::{GlobalGroup, GlobalGroups, ObjectGroups};
use crate::level::trigger::{
    Activated, GlobalTriggers, MultiActivate, SpawnActivate, Trigger, TriggerData, TriggerFunction,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct CollisionTrigger {
    pub(crate) block1_id: u64,
    pub(crate) block2_id: u64,
    pub(crate) target_group: u64,
    pub(crate) activate: bool,
}

#[derive(Component)]
pub(crate) struct CollisionBlock(pub(crate) u64);

type CollisionTriggerSystemParam = (
    Res<'static, Time>,
    Res<'static, GlobalGroups>,
    Res<'static, GlobalTriggers>,
    ResMut<'static, TriggerData>,
    Query<'static, 'static, (&'static ActiveCollider, &'static CollisionBlock)>,
    Query<'static, 'static, &'static CollisionBlock, Without<ActiveCollider>>,
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

impl TriggerFunction for CollisionTrigger {
    fn execute(
        &self,
        world: &mut World,
        entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        progress: f32,
        range: Range<f32>,
    ) {
        if progress != 1. {
            return;
        }

        let system_state: &mut SystemState<CollisionTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let (
            time,
            global_groups,
            global_triggers,
            mut trigger_data,
            collider_query,
            collision_block_query,
            mut group_query,
            trigger_query,
        ) = system_state.get_mut(world);

        let mut collider = None;

        for (active_collider, collision_id) in &collider_query {
            if collision_id.0 == self.block1_id {
                collider = Some((active_collider, false));
                break;
            }
            if collision_id.0 == self.block2_id {
                collider = Some((active_collider, true));
                break;
            }
        }

        let Some((collider, is_block2)) = collider else {
            return;
        };

        let mut collided = false;

        if !is_block2 {
            for collision in collision_block_query
                .iter_many(collider.collided.iter().map(|(entity, _, _, _)| entity))
            {
                if collision.0 == self.block2_id {
                    collided = true;
                    break;
                }
            }
        } else {
            for collision in collision_block_query
                .iter_many(collider.collided.iter().map(|(entity, _, _, _)| entity))
            {
                if collision.0 == self.block1_id {
                    collided = true;
                    break;
                }
            }
        }

        if !collided {
            let next_pos = global_triggers
                .speed_changes
                .pos_for_time(time.elapsed_seconds());
            trigger_data.to_spawn.push((
                entity,
                Trigger(Box::new(self.clone())),
                Vec::new(),
                next_pos..next_pos,
            ));
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
        Box::new(SystemState::<CollisionTriggerSystemParam>::new(world))
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
