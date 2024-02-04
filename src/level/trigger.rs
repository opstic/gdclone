use std::any::{Any, TypeId};

use bevy::ecs::system::SystemState;
use bevy::math::Vec3;
use bevy::prelude::{Component, Entity, EntityWorldMut, Mut, Query, ResMut, Resource, With, World};
use bevy::utils::syncunsafecell::SyncUnsafeCell;
use bevy::utils::{default, hashbrown, HashMap as AHashMap};
use float_next_after::NextAfter;
use indexmap::IndexMap;
use nested_intervals::IntervalSetGeneric;
use ordered_float::OrderedFloat;

// use bevy::log::info_span;
use crate::level::color::{ColorMod, HsvMod, ObjectColorCalculated};
use crate::level::easing::Easing;
use crate::level::player::Player;
use crate::level::transform::Transform2d;
use crate::level::trigger::alpha::AlphaTrigger;
use crate::level::trigger::color::ColorTrigger;
use crate::level::trigger::pulse::PulseTrigger;
use crate::level::trigger::r#move::MoveTrigger;
use crate::level::trigger::rotate::RotateTrigger;
use crate::level::trigger::toggle::ToggleTrigger;
use crate::utils::{u8_to_bool, U64Hash};

mod alpha;
mod color;
mod empty;
mod r#move;
mod pulse;
mod rotate;
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
    fn execute(
        &self,
        world: &mut World,
        entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
    );

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync>;

    fn target_id(&self) -> u64;

    fn duration(&self) -> f32;

    fn exclusive(&self) -> bool;

    fn concrete_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

#[derive(Default, Resource)]
pub(crate) struct TriggerData {
    data: AHashMap<
        TypeId,
        (
            hashbrown::HashMap<u64, u32, U64Hash>,
            SyncUnsafeCell<Box<dyn Any + Send + Sync>>,
        ),
    >,
}

pub(crate) fn process_triggers(world: &mut World) {
    world.resource_scope(|world, mut trigger_data: Mut<TriggerData>| {
        let world_cell = world.as_unsafe_world_cell();

        let system_state: &mut SystemState<(
            ResMut<GlobalTriggers>,
            Query<(&Player, &Transform2d, &TriggerActivator)>,
            Query<(&Trigger, &ObjectColorCalculated)>,
        )> = if let Some((_, cell)) = trigger_data.data.get(&TypeId::of::<World>()) {
            unsafe { &mut *cell.get() }
        } else {
            let system_state: SystemState<(
                ResMut<GlobalTriggers>,
                Query<(&Player, &Transform2d, &TriggerActivator)>,
                Query<(&Trigger, &ObjectColorCalculated)>,
            )> = SystemState::new(unsafe { world_cell.world_mut() });

            trigger_data.data.insert(
                TypeId::of::<World>(),
                (
                    hashbrown::HashMap::with_hasher(U64Hash),
                    SyncUnsafeCell::new(Box::new(system_state)),
                ),
            );

            let (_, cell) = trigger_data.data.get(&TypeId::of::<World>()).unwrap();

            unsafe { &mut *cell.get() }
        }
        .downcast_mut()
        .unwrap();

        let (mut global_triggers, players, triggers) =
            system_state.get_mut(unsafe { world_cell.world_mut() });

        for (player, transform, trigger_activator) in &players {
            let Some(global_trigger_channel) = global_triggers
                .pos_triggers
                .get_mut(&trigger_activator.channel)
            else {
                continue;
            };

            let mut last_translation = player.last_translation;

            if transform.translation.x == 0. {
                last_translation.x = f32::NEG_INFINITY;
            }

            let activate_range =
                OrderedFloat(last_translation.x)..OrderedFloat(transform.translation.x);

            // let span_a = info_span!("interval lookup");
            // let span_b = info_span!("trigger");
            //
            // let query = span_a.in_scope(|| {
            //     global_trigger_channel
            //         .x
            //         .0
            //         .query_overlapping(&activate_range)
            // });

            let query = global_trigger_channel
                .x
                .0
                .query_overlapping(&activate_range);

            for (trigger_range, entity_indices) in query.iter() {
                let trigger_range_length = trigger_range.end.0 - trigger_range.start.0;
                let previous_progress = ((last_translation.x - trigger_range.start.0)
                    / trigger_range_length)
                    .clamp(0., 1.);
                let current_progress = ((transform.translation.x - trigger_range.start.0)
                    / trigger_range_length)
                    .clamp(0., 1.);

                for entity_index in entity_indices {
                    let trigger_entity = global_trigger_channel.x.1[*entity_index as usize];

                    let Ok((trigger, object_color_calculated)) = triggers.get(trigger_entity)
                    else {
                        continue;
                    };

                    if !object_color_calculated.enabled {
                        continue;
                    }

                    // Very unsafe but works for now
                    let world_mut = unsafe { world_cell.world_mut() };

                    let trigger_system_state = if let Some((exclusive_data, system_state)) =
                        trigger_data.data.get_mut(&trigger.0.concrete_type_id())
                    {
                        if let Some(last_index) = exclusive_data.get_mut(&trigger.0.target_id()) {
                            if entity_index < last_index {
                                continue;
                            }
                            if trigger.0.exclusive() {
                                *last_index = *entity_index;
                            }
                        } else if trigger.0.exclusive() {
                            exclusive_data.insert(trigger.0.target_id(), *entity_index);
                        }
                        system_state
                    } else {
                        let mut exclusive_data = hashbrown::HashMap::with_hasher(U64Hash);
                        if trigger.0.exclusive() {
                            exclusive_data.insert(trigger.0.target_id(), *entity_index);
                        }
                        trigger_data.data.insert(
                            trigger.0.concrete_type_id(),
                            (
                                exclusive_data,
                                SyncUnsafeCell::new(trigger.0.create_system_state(world_mut)),
                            ),
                        );

                        &mut trigger_data
                            .data
                            .get_mut(&trigger.0.concrete_type_id())
                            .unwrap()
                            .1
                    };

                    trigger.0.execute(
                        world_mut,
                        trigger_entity,
                        trigger_system_state.get_mut(),
                        previous_progress,
                        current_progress,
                    );

                    // span_b.in_scope(|| {
                    //     trigger.0.execute(
                    //         world_mut,
                    //         trigger_system_state.get_mut(),
                    //         previous_progress,
                    //         current_progress,
                    //     );
                    // });
                }
            }
        }
    });
}

pub(crate) fn insert_trigger_data(
    entity_world_mut: &mut EntityWorldMut,
    object_id: u64,
    object_data: &AHashMap<&[u8], &[u8]>,
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
        899 => {
            let mut trigger = ColorTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = std::str::from_utf8(duration)?.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(target_channel) = object_data.get(b"23".as_ref()) {
                trigger.target_channel = std::str::from_utf8(target_channel)?.parse()?;
            } else {
                trigger.target_channel = 1;
            }
            if let Some(r) = object_data.get(b"7".as_ref()) {
                trigger.target_color[0] =
                    std::str::from_utf8(r)?.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(g) = object_data.get(b"8".as_ref()) {
                trigger.target_color[1] =
                    std::str::from_utf8(g)?.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(b) = object_data.get(b"9".as_ref()) {
                trigger.target_color[2] =
                    std::str::from_utf8(b)?.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(opacity) = object_data.get(b"35".as_ref()) {
                trigger.target_color[3] = std::str::from_utf8(opacity)?.parse()?;
            }
            if let Some(blending) = object_data.get(b"17".as_ref()) {
                trigger.target_blending = u8_to_bool(blending);
            }
            if let Some(copied_hsv) = object_data.get(b"49".as_ref()) {
                trigger.copied_hsv = Some(HsvMod::parse(copied_hsv)?);
            }
            if let Some(copied_channel) = object_data.get(b"50".as_ref()) {
                trigger.copied_channel = std::str::from_utf8(copied_channel)?.parse()?;
            } else {
                trigger.copied_channel = 0;
            }
            if let Some(copy_opacity) = object_data.get(b"60".as_ref()) {
                trigger.copy_opacity = u8_to_bool(copy_opacity);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
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
        1006 => {
            let mut trigger = PulseTrigger::default();
            if let Some(fade_in_duration) = object_data.get(b"45".as_ref()) {
                trigger.fade_in_duration = std::str::from_utf8(fade_in_duration)?.parse()?;
                if trigger.fade_in_duration.is_sign_negative() {
                    trigger.fade_in_duration = 0.;
                }
            }
            if let Some(hold_duration) = object_data.get(b"46".as_ref()) {
                trigger.hold_duration = std::str::from_utf8(hold_duration)?.parse()?;
                if trigger.hold_duration.is_sign_negative() {
                    trigger.hold_duration = 0.;
                }
            }
            if let Some(fade_out_duration) = object_data.get(b"47".as_ref()) {
                trigger.fade_out_duration = std::str::from_utf8(fade_out_duration)?.parse()?;
                if trigger.fade_out_duration.is_sign_negative() {
                    trigger.fade_out_duration = 0.;
                }
            }
            if let Some(target_id) = object_data.get(b"51".as_ref()) {
                trigger.target_id = std::str::from_utf8(target_id)?.parse()?;
            }
            if let Some(target_group) = object_data.get(b"52".as_ref()) {
                trigger.target_is_group = u8_to_bool(target_group);
            }
            let mut mod_mode = false;
            if let Some(hsv_mode) = object_data.get(b"48".as_ref()) {
                mod_mode = u8_to_bool(hsv_mode);
            }
            if mod_mode {
                let mut hsv = HsvMod::default();
                if let Some(targer_hsv) = object_data.get(b"49".as_ref()) {
                    hsv = HsvMod::parse(targer_hsv)?;
                }
                if let Some(color_id) = object_data.get(b"50".as_ref()) {
                    trigger.copied_color_id = std::str::from_utf8(color_id)?.parse()?;
                }
                if !trigger.target_is_group && trigger.copied_color_id == 0 {
                    trigger.copied_color_id = trigger.target_id;
                }
                trigger.color_mod = ColorMod::Hsv(hsv);
            } else {
                let mut color = Vec3::ONE;
                if let Some(r) = object_data.get(b"7".as_ref()) {
                    color[0] = std::str::from_utf8(r)?.parse::<u8>()? as f32 / u8::MAX as f32;
                }
                if let Some(g) = object_data.get(b"8".as_ref()) {
                    color[1] = std::str::from_utf8(g)?.parse::<u8>()? as f32 / u8::MAX as f32;
                }
                if let Some(b) = object_data.get(b"9".as_ref()) {
                    color[2] = std::str::from_utf8(b)?.parse::<u8>()? as f32 / u8::MAX as f32;
                }
                trigger.color_mod = ColorMod::Color(color);
            }
            if let Some(base_only) = object_data.get(b"65".as_ref()) {
                trigger.base_only = u8_to_bool(base_only);
            }
            if let Some(detail_only) = object_data.get(b"66".as_ref()) {
                trigger.detail_only = u8_to_bool(detail_only);
            }
            if let Some(exclusive) = object_data.get(b"86".as_ref()) {
                trigger.exclusive = u8_to_bool(exclusive);
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
        1346 => {
            let mut trigger = RotateTrigger::default();
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
            if let Some(center_group) = object_data.get(b"71".as_ref()) {
                trigger.center_group = std::str::from_utf8(center_group)?.parse()?;
            }
            if let Some(degrees) = object_data.get(b"68".as_ref()) {
                trigger.degrees = -std::str::from_utf8(degrees)?.parse::<f32>()?.to_radians();
            }
            if let Some(times360) = object_data.get(b"69".as_ref()) {
                trigger.times360 = -std::str::from_utf8(times360)?.parse()?;
            }
            if let Some(lock_rotation) = object_data.get(b"70".as_ref()) {
                trigger.lock_rotation = u8_to_bool(lock_rotation);
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
    let mut speed_change_query = world.query::<(Entity, &SpeedChange, &Transform2d)>();

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
        world.query_filtered::<(Entity, &Trigger, &Transform2d), With<PosActivate>>();

    let mut trigger_entities = Vec::new();
    let mut trigger_intervals = Vec::new();

    let mut sorted_triggers = Vec::new();

    for (entity, _, transform) in triggers_query.iter(world) {
        sorted_triggers.push((OrderedFloat(transform.translation.x), entity));
    }

    sorted_triggers.sort_unstable();

    for (entity, trigger, transform) in
        triggers_query.iter_many(world, sorted_triggers.iter().map(|(_, entity)| entity))
    {
        let trigger_start_pos = transform.translation.x;
        let mut trigger_end_pos = if trigger.0.duration() > 0. {
            let start_pos_time = global_triggers
                .speed_changes
                .time_for_pos(transform.translation.x);
            global_triggers
                .speed_changes
                .pos_for_time(start_pos_time + trigger.0.duration())
        } else {
            trigger_start_pos.next_after(f32::INFINITY)
        };

        if trigger_start_pos >= trigger_end_pos {
            trigger_end_pos = trigger_start_pos.next_after(f32::INFINITY);
        }

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
