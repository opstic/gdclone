use std::any::TypeId;

use bevy::ecs::system::SystemState;
use bevy::prelude::{
    Component, Entity, EntityWorldMut, Has, Query, ResMut, Resource, Transform, World,
};
use bevy::utils::petgraph::matrix_graph::Zero;
use bevy::utils::{default, HashMap};
use float_next_after::NextAfter;
use indexmap::IndexMap;
use nested_intervals::IntervalSetGeneric;
use ordered_float::OrderedFloat;

use crate::level::easing::Easing;
use crate::level::player::Player;
use crate::level::trigger::alpha::AlphaTrigger;
use crate::level::trigger::r#move::MoveTrigger;
use crate::level::trigger::toggle::ToggleTrigger;
use crate::utils::{u8_to_bool, U64Hash};

mod alpha;
mod r#move;
mod toggle;

#[derive(Default, Resource)]
pub(crate) struct GlobalTriggers {
    pub(crate) speed_changes: SpeedChanges,
    pos_triggers: IndexMap<u64, GlobalTriggerChannel, U64Hash>,
    spawn_triggers: IndexMap<u64, Vec<Entity>, U64Hash>,
}

#[derive(Debug, Default)]
pub(crate) struct SpeedChanges(Vec<(OrderedFloat<f32>, SpeedChangeData)>);

#[derive(Debug)]
pub(crate) struct SpeedChangeData {
    speed_per_sec: f32,
    time_at_pos: f32,
    pub(crate) entity: Entity,
}

#[derive(Component)]
pub(crate) struct SpeedChange {
    pub(crate) forward_velocity: f32,
    pub(crate) speed: f32,
}

impl SpeedChanges {
    fn initialize(&mut self) {
        self.0.sort_unstable_by_key(|(index, _)| *index);

        let mut last_pos = f32::NEG_INFINITY;
        let mut to_remove = Vec::new();

        for (index, (pos, _)) in self.0.iter().enumerate() {
            if last_pos == pos.0 {
                to_remove.push(index);
                continue;
            }
            last_pos = pos.0;
        }

        for index in to_remove.iter().rev() {
            self.0.remove(*index);
        }

        let mut index = 1;

        while index < self.0.len() {
            let (first, second) = self.0.split_at_mut(index);
            let last_change = unsafe { first.get_unchecked(first.len() - 1) };
            let this_change = unsafe { second.get_unchecked_mut(0) };

            this_change.1.time_at_pos = (this_change.0 .0 - last_change.0 .0)
                / last_change.1.speed_per_sec
                + last_change.1.time_at_pos;

            index += 1;
        }
    }

    pub(crate) fn speed_data_at_pos(&self, pos: f32) -> &(OrderedFloat<f32>, SpeedChangeData) {
        let index = self
            .0
            .binary_search_by_key(&OrderedFloat(pos), |(pos, _)| *pos)
            .unwrap_or_else(|index| index.saturating_sub(1));

        &self.0[index]
    }

    pub(crate) fn speed_data_at_time(&self, time: f32) -> &(OrderedFloat<f32>, SpeedChangeData) {
        let index = self
            .0
            .binary_search_by_key(&OrderedFloat(time), |(_, speed_change_data)| {
                OrderedFloat(speed_change_data.time_at_pos)
            })
            .unwrap_or_else(|index| index.saturating_sub(1));

        &self.0[index]
    }

    fn time_for_pos(&self, pos: f32) -> f32 {
        let (speed_change_pos, speed_change_data) = self.speed_data_at_pos(pos);

        speed_change_data.time_at_pos + (pos - speed_change_pos.0) / speed_change_data.speed_per_sec
    }

    fn pos_for_time(&self, time: f32) -> f32 {
        let (speed_change_pos, speed_change_data) = self.speed_data_at_time(time);

        speed_change_pos.0
            + (time - speed_change_data.time_at_pos) * speed_change_data.speed_per_sec
    }
}

#[derive(Debug)]
struct GlobalTriggerChannel {
    x: (IntervalSetGeneric<OrderedFloat<f32>>, Vec<Entity>),
    // y: (IntervalSetGeneric<OrderedFloat<f32>>, Vec<Entity>),
}

#[derive(Default, Component)]
pub(crate) struct TriggerActivator {
    channel: u64,
}

#[derive(Component)]
pub(crate) struct TouchActivate;

#[derive(Component)]
pub(crate) struct SpawnActivate;

#[derive(Component)]
pub(crate) struct PosActivate;

#[derive(Component)]
pub(crate) struct MultiActivate;

#[derive(Component)]
pub(crate) struct Trigger(Box<dyn TriggerFunction>);

pub(crate) trait TriggerFunction: Send + Sync + 'static {
    fn execute(&self, world: &mut World, previous_progress: f32, progress: f32);

    fn duration(&self) -> f32;

    fn exclusive(&self) -> bool;

    fn concrete_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub(crate) fn process_triggers(world: &mut World) {
    let mut system_state: SystemState<(
        ResMut<GlobalTriggers>,
        Query<(&Player, &Transform, &TriggerActivator)>,
        Query<&Trigger>,
    )> = SystemState::new(world);

    let world_cell = world.as_unsafe_world_cell();

    let (mut global_triggers, players, triggers) =
        system_state.get_mut(unsafe { world_cell.world_mut() });

    for (player, transform, trigger_activator) in &players {
        let Some(global_trigger_channel) = global_triggers
            .pos_triggers
            .get_mut(&trigger_activator.channel)
        else {
            continue;
        };

        let mut activate_range =
            OrderedFloat(player.last_translation.x)..OrderedFloat(transform.translation.x);

        if player.last_translation.x.is_zero() {
            activate_range.start = OrderedFloat(f32::NEG_INFINITY);
        }

        for (trigger_range, entity_indices) in global_trigger_channel
            .x
            .0
            .query_overlapping(&activate_range)
            .iter()
        {
            let trigger_range_length = trigger_range.end.0 - trigger_range.start.0;
            let previous_progress =
                (player.last_translation.x - trigger_range.start.0).max(0.) / trigger_range_length;
            let current_progress =
                ((transform.translation.x - trigger_range.start.0) / trigger_range_length).min(1.);

            for entity_index in entity_indices {
                let trigger_entity = global_trigger_channel.x.1[*entity_index as usize];

                let Ok(trigger) = triggers.get(trigger_entity) else {
                    continue;
                };

                // Very unsafe but works for now
                let world_mut = unsafe { world_cell.world_mut() };

                trigger
                    .0
                    .execute(world_mut, previous_progress, current_progress);
            }
        }
    }
}

pub(crate) fn insert_trigger_data(
    entity_world_mut: &mut EntityWorldMut,
    object_id: u64,
    object_data: &HashMap<&[u8], &[u8]>,
) -> Result<(), anyhow::Error> {
    match object_id {
        200 => {
            // Known as "0.5x"
            entity_world_mut.insert(SpeedChange {
                forward_velocity: 5.98 * 60.,
                speed: 0.7,
            });
            entity_world_mut.insert(TouchActivate);
            return Ok(());
        }
        201 => {
            // Known as "1x"
            entity_world_mut.insert(SpeedChange {
                forward_velocity: 5.77 * 60.,
                speed: 0.9,
            });
            entity_world_mut.insert(TouchActivate);
            return Ok(());
        }
        202 => {
            // Known as "2x"
            entity_world_mut.insert(SpeedChange {
                forward_velocity: 5.87 * 60.,
                speed: 1.1,
            });
            entity_world_mut.insert(TouchActivate);
            return Ok(());
        }
        203 => {
            // Known as "3x"
            entity_world_mut.insert(SpeedChange {
                forward_velocity: 6. * 60.,
                speed: 1.3,
            });
            entity_world_mut.insert(TouchActivate);
            return Ok(());
        }
        1334 => {
            // Known as "4x"
            entity_world_mut.insert(SpeedChange {
                forward_velocity: 6. * 60.,
                speed: 1.6,
            });
            entity_world_mut.insert(TouchActivate);
            return Ok(());
        }
        901 => {
            let mut trigger = MoveTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = std::str::from_utf8(duration)?.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(easing) = object_data.get(b"30".as_ref()) {
                let id = std::str::from_utf8(easing)?.parse()?;
                let rate = object_data
                    .get(b"85".as_ref())
                    .map(|b| std::str::from_utf8(b).unwrap().parse().unwrap());
                trigger.easing = Easing::from_id(id, rate)
            }
            if let Some(target_group) = object_data.get(b"51".as_ref()) {
                trigger.target_group = std::str::from_utf8(target_group)?.parse()?;
            }
            if let Some(x_offset) = object_data.get(b"28".as_ref()) {
                trigger.offset.x = std::str::from_utf8(x_offset)?.parse()?;
            }
            if let Some(y_offset) = object_data.get(b"29".as_ref()) {
                trigger.offset.y = std::str::from_utf8(y_offset)?.parse()?;
            }
            if let Some(lock_x) = object_data.get(b"58".as_ref()) {
                trigger.lock.x = u8_to_bool(lock_x);
            }
            if let Some(lock_y) = object_data.get(b"59".as_ref()) {
                trigger.lock.y = u8_to_bool(lock_y);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1007 => {
            let mut trigger = AlphaTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = std::str::from_utf8(duration)?.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(target_group) = object_data.get(b"51".as_ref()) {
                trigger.target_group = std::str::from_utf8(target_group)?.parse()?;
            }
            if let Some(target_opacity) = object_data.get(b"35".as_ref()) {
                trigger.target_opacity = std::str::from_utf8(target_opacity)?.parse()?;
            } else {
                trigger.target_opacity = 1.;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1049 => {
            let mut trigger = ToggleTrigger::default();
            if let Some(target_group) = object_data.get(b"51".as_ref()) {
                trigger.target_group = std::str::from_utf8(target_group)?.parse()?;
            }
            if let Some(activate) = object_data.get(b"56".as_ref()) {
                trigger.activate = u8_to_bool(activate);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        _ => return Ok(()),
    }
    let touch_triggered = object_data
        .get(b"11".as_ref())
        .map(|b| u8_to_bool(b))
        .unwrap_or_default();
    let spawn_triggered = object_data
        .get(b"62".as_ref())
        .map(|b| u8_to_bool(b))
        .unwrap_or_default();
    if touch_triggered {
        entity_world_mut.insert(TouchActivate);
    } else if spawn_triggered {
        entity_world_mut.insert(SpawnActivate);
    } else {
        entity_world_mut.insert(PosActivate);
    }
    Ok(())
}

pub(crate) fn construct_trigger_index(world: &mut World) {
    let mut speed_changes = SpeedChanges::default();

    // Start by indexing speed changes
    let mut speed_change_query = world.query::<(Entity, &SpeedChange, &Transform)>();

    for (entity, speed_change, transform) in speed_change_query.iter(world) {
        speed_changes.0.push((
            OrderedFloat(transform.translation.x),
            SpeedChangeData {
                speed_per_sec: speed_change.speed * speed_change.forward_velocity,
                // Calculate time at pos in SpeedChanges::initialize()
                time_at_pos: 0.,
                entity,
            },
        ));
    }

    speed_changes.initialize();

    let mut global_triggers = GlobalTriggers {
        speed_changes,
        ..default()
    };

    // Then get each of the position activated triggers and precompute their range to create a timeline
    let mut triggers_query =
        world.query_filtered::<(Entity, &Trigger, &Transform), Has<PosActivate>>();

    let mut trigger_entities = Vec::new();
    let mut trigger_intervals = Vec::new();

    for (entity, trigger, transform) in triggers_query.iter(world) {
        let trigger_start_pos = transform.translation.x;
        let trigger_end_pos = if trigger.0.duration() > 0. {
            let start_pos_time = global_triggers
                .speed_changes
                .time_for_pos(transform.translation.x);
            global_triggers
                .speed_changes
                .pos_for_time(start_pos_time + trigger.0.duration())
        } else {
            trigger_start_pos.next_after(f32::INFINITY)
        };

        trigger_entities.push(entity);
        trigger_intervals.push(OrderedFloat(trigger_start_pos)..OrderedFloat(trigger_end_pos));
    }

    let interval_ids: Vec<u32> = trigger_intervals
        .iter()
        .enumerate()
        .map(|(index, _)| index as u32)
        .collect();

    global_triggers.pos_triggers.insert(
        0,
        GlobalTriggerChannel {
            x: (
                IntervalSetGeneric::new_with_ids(&trigger_intervals, &interval_ids).unwrap(),
                trigger_entities,
            ),
        },
    );

    world.insert_resource(global_triggers);
}
