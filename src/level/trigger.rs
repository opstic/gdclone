use std::any::{Any, TypeId};
use std::cell::UnsafeCell;
use std::ops::Range;

use bevy::ecs::system::SystemState;
use bevy::math::Vec3A;
use bevy::prelude::{Component, Entity, EntityWorldMut, Query, ResMut, Resource, With, World};
use bevy::utils::syncunsafecell::SyncUnsafeCell;
use bevy::utils::{default, hashbrown, HashMap as AHashMap};
use dyn_clone::DynClone;
use float_next_after::NextAfter;
use indexmap::IndexMap;
use nested_intervals::IntervalSetGeneric;
use ordered_float::OrderedFloat;

use crate::level::collision::{ActiveCollider, GlobalHitbox, Hitbox};
// use bevy::log::info_span;
use crate::level::color::{ColorMod, HsvMod, ObjectColorCalculated};
use crate::level::easing::Easing;
use crate::level::group::ObjectGroups;
use crate::level::player::Player;
use crate::level::transform::{GlobalTransform2d, Transform2d};
use crate::level::trigger::alpha::AlphaTrigger;
use crate::level::trigger::color::ColorTrigger;
use crate::level::trigger::count::CountTrigger;
use crate::level::trigger::follow::FollowTrigger;
use crate::level::trigger::instant_count::{InstantCountMode, InstantCountTrigger};
use crate::level::trigger::pickup::{PickupTrigger, PickupValues};
use crate::level::trigger::pulse::PulseTrigger;
use crate::level::trigger::r#move::MoveTrigger;
use crate::level::trigger::rotate::RotateTrigger;
use crate::level::trigger::spawn::SpawnTrigger;
use crate::level::trigger::stop::StopTrigger;
use crate::level::trigger::toggle::ToggleTrigger;
use crate::utils::{str_to_bool, U64Hash};

mod alpha;
mod color;
mod count;
mod empty;
mod follow;
mod instant_count;
mod r#move;
mod pickup;
mod pulse;
mod rotate;
mod spawn;
mod stop;
mod toggle;

#[derive(Default, Resource)]
pub(crate) struct GlobalTriggers {
    pub(crate) speed_changes: SpeedChanges,
    pos_triggers: IndexMap<u64, GlobalTriggerChannel, U64Hash>,
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

    pub(crate) fn time_for_pos(&self, pos: f32) -> f32 {
        let (speed_change_pos, speed_change_data) = self.speed_data_at_pos(pos);

        speed_change_data.time_at_pos + (pos - speed_change_pos.0) / speed_change_data.speed_per_sec
    }

    pub(crate) fn pos_for_time(&self, time: f32) -> f32 {
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
pub(crate) struct Activated;

#[derive(Clone, Component)]
pub(crate) struct Trigger(Box<dyn TriggerFunction>);

pub(crate) trait TriggerFunction: DynClone + Send + Sync + 'static {
    fn execute(
        &self,
        world: &mut World,
        entity: Entity,
        trigger_index: u32,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
        range: Range<f32>,
    );

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync>;

    fn target_id(&self) -> u64;

    fn duration(&self) -> f32;

    fn exclusive(&self) -> bool;

    fn post(&self) -> bool;

    fn concrete_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

dyn_clone::clone_trait_object!(TriggerFunction);

#[derive(Default, Resource)]
pub(crate) struct TriggerData {
    stopped: IndexMap<u64, u32, U64Hash>,
    data: AHashMap<
        TypeId,
        (
            hashbrown::HashMap<u64, f32, U64Hash>,
            SyncUnsafeCell<Box<dyn Any + Send + Sync>>,
        ),
    >,
    to_spawn: Vec<(Entity, Trigger, Vec<u64>, u32, Range<f32>)>,
    spawned: Vec<(Entity, Trigger, Vec<u64>, u32, Range<f32>)>,
}

pub(crate) fn process_triggers(world: &mut World) {
    let world_cell = world.as_unsafe_world_cell();

    let mut trigger_data = unsafe { world_cell.world_mut() }.resource_mut::<TriggerData>();

    let system_state: &mut SystemState<(
        ResMut<GlobalTriggers>,
        Query<(&Player, &Transform2d, &TriggerActivator)>,
        Query<(&Trigger, &ObjectGroups, &ObjectColorCalculated)>,
    )> = if let Some((_, cell)) = trigger_data.data.get(&TypeId::of::<World>()) {
        unsafe { &mut *cell.get() }
    } else {
        let system_state: SystemState<(
            ResMut<GlobalTriggers>,
            Query<(&Player, &Transform2d, &TriggerActivator)>,
            Query<(&Trigger, &ObjectGroups, &ObjectColorCalculated)>,
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

        let mut post_triggers = Vec::new();

        let query = global_trigger_channel
            .x
            .0
            .query_overlapping(&activate_range);

        for (trigger_range, entity_indices) in query.iter() {
            let trigger_range_length = trigger_range.end.0 - trigger_range.start.0;
            let previous_progress =
                ((last_translation.x - trigger_range.start.0) / trigger_range_length).clamp(0., 1.);
            let current_progress = ((transform.translation.x - trigger_range.start.0)
                / trigger_range_length)
                .clamp(0., 1.);

            'trigger_loop: for entity_index in entity_indices {
                let trigger_entity = global_trigger_channel.x.1[*entity_index as usize];

                let Ok((trigger, object_groups, object_color_calculated)) =
                    triggers.get(trigger_entity)
                else {
                    continue;
                };

                if !object_color_calculated.enabled {
                    continue;
                }

                for (stopped_group, stop_index) in &trigger_data.stopped {
                    if object_groups
                        .groups
                        .iter()
                        .any(|group_id| group_id == stopped_group)
                        && entity_index < stop_index
                    {
                        continue 'trigger_loop;
                    }
                }

                let range = trigger_range.start.0..trigger_range.end.0;

                if trigger.0.post() {
                    post_triggers.push((
                        trigger.clone(),
                        trigger_entity,
                        *entity_index,
                        previous_progress,
                        current_progress,
                        range,
                    ));
                    continue;
                }

                // Very unsafe but works for now
                let world_mut = unsafe { world_cell.world_mut() };

                run_trigger(
                    trigger,
                    world_mut,
                    trigger_entity,
                    *entity_index,
                    previous_progress,
                    current_progress,
                    range,
                    &mut trigger_data,
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

        let trigger_data_cell = UnsafeCell::new(&mut *trigger_data);

        unsafe { &mut **trigger_data_cell.get() }
            .spawned
            .append(&mut unsafe { &mut **trigger_data_cell.get() }.to_spawn);

        unsafe { &mut **trigger_data_cell.get() }.spawned.retain(
            |(entity, trigger, groups, entity_index, range)| {
                let trigger_data = unsafe { &mut **trigger_data_cell.get() };

                let trigger_range_length = range.end - range.start;
                let mut previous_progress =
                    ((last_translation.x - range.start) / trigger_range_length).clamp(0., 1.);
                let current_progress =
                    ((transform.translation.x - range.start) / trigger_range_length).clamp(0., 1.);

                if previous_progress == 1. && current_progress == 1. {
                    previous_progress = 0.;
                }

                for (stopped_group, stop_index) in &trigger_data.stopped {
                    if groups.iter().any(|group_id| group_id == stopped_group)
                        && entity_index < stop_index
                    {
                        return false;
                    }
                }

                if trigger.0.post() {
                    post_triggers.push((
                        trigger.clone(),
                        *entity,
                        *entity_index,
                        previous_progress,
                        current_progress,
                        range.clone(),
                    ));
                    return current_progress != 1.;
                }

                // Very unsafe but works for now
                let world_mut = unsafe { world_cell.world_mut() };

                run_trigger(
                    trigger,
                    world_mut,
                    *entity,
                    *entity_index,
                    previous_progress,
                    current_progress,
                    range.clone(),
                    trigger_data,
                );

                current_progress != 1.
            },
        );

        for (trigger, entity, entity_index, previous_progress, current_progress, range) in
            post_triggers
        {
            // Very unsafe but works for now
            let world_mut = unsafe { world_cell.world_mut() };

            run_trigger(
                &trigger,
                world_mut,
                entity,
                entity_index,
                previous_progress,
                current_progress,
                range,
                &mut trigger_data,
            );
        }
    }
}

fn run_trigger(
    trigger: &Trigger,
    world: &mut World,
    entity: Entity,
    entity_index: u32,
    previous_progress: f32,
    current_progress: f32,
    range: Range<f32>,
    trigger_data: &mut TriggerData,
) {
    let trigger_system_state = if let Some((exclusive_data, system_state)) =
        trigger_data.data.get_mut(&trigger.0.concrete_type_id())
    {
        if let Some(last_pos) = exclusive_data.get_mut(&trigger.0.target_id()) {
            if range.start < *last_pos {
                return;
            }
            if trigger.0.exclusive() {
                *last_pos = range.start;
            }
        } else if trigger.0.exclusive() {
            exclusive_data.insert(trigger.0.target_id(), range.start);
        }
        system_state
    } else {
        let mut exclusive_data = hashbrown::HashMap::with_hasher(U64Hash);
        if trigger.0.exclusive() {
            exclusive_data.insert(trigger.0.target_id(), range.start);
        }
        trigger_data.data.insert(
            trigger.0.concrete_type_id(),
            (
                exclusive_data,
                SyncUnsafeCell::new(trigger.0.create_system_state(world)),
            ),
        );

        &mut trigger_data
            .data
            .get_mut(&trigger.0.concrete_type_id())
            .unwrap()
            .1
    };

    trigger.0.execute(
        world,
        entity,
        entity_index,
        trigger_system_state.get_mut(),
        previous_progress,
        current_progress,
        range,
    );
}

pub(crate) fn insert_trigger_data(
    entity_world_mut: &mut EntityWorldMut,
    object_id: u64,
    object_data: &AHashMap<&str, &str>,
) -> Result<(), anyhow::Error> {
    match object_id {
        200 | 201 | 202 | 203 | 1334 => {
            if let Some(editor_preview) = object_data.get("13") {
                if !str_to_bool(editor_preview) {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
        _ => (),
    }
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
        29 | 30 | 105 | 221 | 717 | 718 | 743 | 744 | 899 => {
            let mut trigger = ColorTrigger::default();
            if let Some(duration) = object_data.get("10") {
                trigger.duration = duration.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(target_channel) = object_data.get("23") {
                trigger.target_channel = target_channel.parse()?;
            } else {
                trigger.target_channel = 1;
            }
            if trigger.target_channel > 999 {
                match trigger.target_channel {
                    1000 | 1001 | 1002 | 1003 | 1004 | 1009 => (),
                    _ => return Ok(()),
                }
            }
            trigger.target_channel = match object_id {
                221 => 1,
                717 => 2,
                718 => 3,
                743 => 4,
                29 => 1000,
                30 => 1001,
                744 => 1003,
                105 => 1004,
                _ => trigger.target_channel,
            };
            if let Some(r) = object_data.get("7") {
                trigger.target_color[0] = r.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(g) = object_data.get("8") {
                trigger.target_color[1] = g.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(b) = object_data.get("9") {
                trigger.target_color[2] = b.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(opacity) = object_data.get("35") {
                trigger.target_color[3] = opacity.parse()?;
            }
            if let Some(blending) = object_data.get("17") {
                trigger.target_blending = str_to_bool(blending);
            }
            if let Some(copied_hsv) = object_data.get("49") {
                trigger.copied_hsv = Some(HsvMod::parse(copied_hsv)?);
            }
            if let Some(copied_channel) = object_data.get("50") {
                trigger.copied_channel = copied_channel.parse()?;
            } else {
                trigger.copied_channel = 0;
            }
            if let Some(copy_opacity) = object_data.get("60") {
                trigger.copy_opacity = str_to_bool(copy_opacity);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        901 => {
            let mut trigger = MoveTrigger::default();
            if let Some(duration) = object_data.get("10") {
                trigger.duration = duration.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(easing) = object_data.get("30") {
                let id = easing.parse()?;
                let rate = object_data.get("85").map(|b| b.parse()).transpose()?;
                trigger.easing = Easing::from_id(id, rate)
            }
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(x_offset) = object_data.get("28") {
                trigger.offset.x = x_offset.parse()?;
            }
            if let Some(y_offset) = object_data.get("29") {
                trigger.offset.y = y_offset.parse()?;
            }
            if let Some(lock_x) = object_data.get("58") {
                trigger.lock.x = str_to_bool(lock_x);
            }
            if let Some(lock_y) = object_data.get("59") {
                trigger.lock.y = str_to_bool(lock_y);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1006 => {
            let mut trigger = PulseTrigger::default();
            if let Some(fade_in_duration) = object_data.get("45") {
                trigger.fade_in_duration = fade_in_duration.parse()?;
                if trigger.fade_in_duration.is_sign_negative() {
                    trigger.fade_in_duration = 0.;
                }
            }
            if let Some(hold_duration) = object_data.get("46") {
                trigger.hold_duration = hold_duration.parse()?;
                if trigger.hold_duration.is_sign_negative() {
                    trigger.hold_duration = 0.;
                }
            }
            if let Some(fade_out_duration) = object_data.get("47") {
                trigger.fade_out_duration = fade_out_duration.parse()?;
                if trigger.fade_out_duration.is_sign_negative() {
                    trigger.fade_out_duration = 0.;
                }
            }
            if let Some(target_id) = object_data.get("51") {
                trigger.target_id = target_id.parse()?;
            }
            if let Some(target_group) = object_data.get("52") {
                trigger.target_is_group = str_to_bool(target_group);
            }
            let mut mod_mode = false;
            if let Some(hsv_mode) = object_data.get("48") {
                mod_mode = str_to_bool(hsv_mode);
            }
            if mod_mode {
                let mut hsv = HsvMod::default();
                if let Some(targer_hsv) = object_data.get("49") {
                    hsv = HsvMod::parse(targer_hsv)?;
                }
                if let Some(color_id) = object_data.get("50") {
                    trigger.copied_color_id = color_id.parse()?;
                }
                if !trigger.target_is_group && trigger.copied_color_id == 0 {
                    trigger.copied_color_id = trigger.target_id;
                }
                trigger.color_mod = ColorMod::Hsv(hsv);
            } else {
                let mut color = Vec3A::ONE;
                if let Some(r) = object_data.get("7") {
                    color[0] = r.parse::<u8>()? as f32 / u8::MAX as f32;
                }
                if let Some(g) = object_data.get("8") {
                    color[1] = g.parse::<u8>()? as f32 / u8::MAX as f32;
                }
                if let Some(b) = object_data.get("9") {
                    color[2] = b.parse::<u8>()? as f32 / u8::MAX as f32;
                }
                trigger.color_mod = ColorMod::Color(color);
            }
            if let Some(base_only) = object_data.get("65") {
                trigger.base_only = str_to_bool(base_only);
            }
            if let Some(detail_only) = object_data.get("66") {
                trigger.detail_only = str_to_bool(detail_only);
            }
            if let Some(exclusive) = object_data.get("86") {
                trigger.exclusive = str_to_bool(exclusive);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1007 => {
            let mut trigger = AlphaTrigger::default();
            if let Some(duration) = object_data.get("10") {
                trigger.duration = duration.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(target_opacity) = object_data.get("35") {
                trigger.target_opacity = target_opacity.parse()?;
            } else {
                trigger.target_opacity = 1.;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1049 => {
            let mut trigger = ToggleTrigger::default();
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(activate) = object_data.get("56") {
                trigger.activate = str_to_bool(activate);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1268 => {
            let mut trigger = SpawnTrigger::default();
            if let Some(delay) = object_data.get("63") {
                trigger.delay = delay.parse()?;
                if trigger.delay.is_sign_negative() {
                    trigger.delay = 0.;
                }
            }
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1346 => {
            let mut trigger = RotateTrigger::default();
            if let Some(duration) = object_data.get("10") {
                trigger.duration = duration.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(easing) = object_data.get("30") {
                let id = easing.parse()?;
                let rate = object_data.get("85").map(|b| b.parse()).transpose()?;
                trigger.easing = Easing::from_id(id, rate)
            }
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(center_group) = object_data.get("71") {
                trigger.center_group = center_group.parse()?;
            }
            if let Some(degrees) = object_data.get("68") {
                trigger.degrees = -degrees.parse::<f32>()?.to_radians();
            }
            if let Some(times360) = object_data.get("69") {
                trigger.times360 = -times360.parse()?;
            }
            if let Some(lock_rotation) = object_data.get("70") {
                trigger.lock_rotation = str_to_bool(lock_rotation);
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1347 => {
            let mut trigger = FollowTrigger::default();
            if let Some(duration) = object_data.get("10") {
                trigger.duration = duration.parse()?;
                if trigger.duration.is_sign_negative() {
                    trigger.duration = 0.;
                }
            }
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(follow_group) = object_data.get("71") {
                trigger.follow_group = follow_group.parse()?;
            }
            if let Some(scale_x) = object_data.get("72") {
                trigger.scale.x = scale_x.parse()?;
            }
            if let Some(scale_y) = object_data.get("73") {
                trigger.scale.y = scale_y.parse()?;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1611 => {
            let mut trigger = CountTrigger::default();
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(activate) = object_data.get("56") {
                trigger.activate = str_to_bool(activate);
            }
            if let Some(target_count) = object_data.get("77") {
                trigger.target_count = target_count.parse()?;
            }
            if let Some(item_id) = object_data.get("80") {
                trigger.item_id = item_id.parse()?;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1616 => {
            let mut trigger = StopTrigger::default();
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1811 => {
            let mut trigger = InstantCountTrigger::default();
            if let Some(target_group) = object_data.get("51") {
                trigger.target_group = target_group.parse()?;
            }
            if let Some(activate) = object_data.get("56") {
                trigger.activate = str_to_bool(activate);
            }
            if let Some(target_count) = object_data.get("77") {
                trigger.target_count = target_count.parse()?;
            }
            if let Some(item_id) = object_data.get("80") {
                trigger.item_id = item_id.parse()?;
            }
            if let Some(mode) = object_data.get("88") {
                trigger.mode = match mode.parse()? {
                    0 => InstantCountMode::Equal,
                    1 => InstantCountMode::Larger,
                    2 => InstantCountMode::Smaller,
                    _ => unreachable!(),
                }
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        1816 => {
            if let Some(dynamic) = object_data.get("94") {
                if str_to_bool(dynamic) {
                    entity_world_mut.insert(ActiveCollider::default());
                }
            }
            return Ok(());
        }
        1817 => {
            let mut trigger = PickupTrigger::default();
            if let Some(count) = object_data.get("77") {
                trigger.count = count.parse()?;
            }
            if let Some(item_id) = object_data.get("80") {
                trigger.item_id = item_id.parse()?;
            }
            entity_world_mut.insert(Trigger(Box::new(trigger)));
        }
        _ => return Ok(()),
    }

    let touch_triggered = object_data
        .get("11")
        .map(|b| str_to_bool(b))
        .unwrap_or_default();
    let spawn_triggered = object_data
        .get("62")
        .map(|b| str_to_bool(b))
        .unwrap_or_default();
    if touch_triggered {
        entity_world_mut.insert(TouchActivate);
    } else if spawn_triggered {
        entity_world_mut.insert(SpawnActivate);
    } else {
        entity_world_mut.insert(PosActivate);
    }

    if let Some(multi_activate) = object_data.get("87") {
        if str_to_bool(multi_activate) {
            entity_world_mut.insert(MultiActivate);
        }
    }

    Ok(())
}

pub(crate) fn construct_trigger_index(world: &mut World) {
    let mut speed_changes = SpeedChanges::default();

    // Start by indexing speed changes
    let mut speed_change_query = world.query::<(Entity, &SpeedChange, &Transform2d, &Hitbox)>();

    for (entity, speed_change, transform, hitbox) in speed_change_query.iter(world) {
        let global_transform = GlobalTransform2d::from(*transform);
        let global_hitbox = GlobalHitbox::from((hitbox, transform, &global_transform));
        speed_changes.0.push((
            OrderedFloat(global_hitbox.aabb.x),
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
    world.init_resource::<PickupValues>();
}
