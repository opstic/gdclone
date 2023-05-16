use crate::level::easing::Easing;
use crate::level::object::Object;
use crate::level::trigger::alpha::AlphaTrigger;
use crate::level::trigger::color::ColorTrigger;
use crate::level::trigger::r#move::MoveTrigger;
use crate::level::trigger::rotate::RotateTrigger;
use crate::level::trigger::toggle::ToggleTrigger;
use crate::level::Groups;
use crate::utils::u8_to_bool;

use bevy::prelude::{
    Camera2d, Commands, Component, Entity, Mut, Query, Res, ResMut, Resource, SystemSet, Transform,
    With, Without, World,
};
use bevy::utils::HashMap;
use dyn_clone::DynClone;
use std::any::{Any, TypeId};
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) mod alpha;
pub(crate) mod color;
pub(crate) mod r#move;
pub(crate) mod rotate;
pub(crate) mod toggle;

#[derive(Component)]
pub(crate) struct TouchActivate;

#[derive(Component)]
pub(crate) struct SpawnActivate;

#[derive(Component)]
pub(crate) struct XPosActivate;

#[derive(Component)]
pub(crate) struct MultiActivate;

#[derive(Component)]
pub(crate) struct TriggerActivated;

#[derive(Component)]
pub(crate) struct TriggerInProgress;

#[derive(Default, Resource)]
pub(crate) struct ExecutingTriggers(
    pub(crate) HashMap<u32, Vec<(Entity, Box<dyn TriggerFunction>)>>,
);

#[derive(Component)]
pub(crate) struct Trigger(pub(crate) Box<dyn TriggerFunction>);

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub(crate) enum TriggerSystems {
    ActivateTriggers,
    ExecuteTriggers,
}

#[derive(Clone, Default)]
pub(crate) struct TriggerDuration {
    elapsed: Duration,
    duration: Duration,
}

impl TriggerDuration {
    pub(crate) fn new(duration: Duration) -> Self {
        Self {
            elapsed: Duration::ZERO,
            duration,
        }
    }

    fn tick(&mut self, delta: Duration) {
        self.elapsed = self.elapsed.saturating_add(delta);
    }

    fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn fraction_progress(&self) -> f32 {
        if self.elapsed() >= self.duration {
            return 1.;
        }
        (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()).fract() as f32
    }

    fn completed(&self) -> bool {
        self.elapsed > self.duration
    }
}

pub(crate) trait TriggerFunction: Send + Sync + DynClone {
    fn execute(&mut self, world: &mut World);

    fn get_target_group(&self) -> u32;

    fn done_executing(&self) -> bool;
}

dyn_clone::clone_trait_object!(TriggerFunction);

pub(crate) fn activate_xpos_triggers(
    commands: Commands,
    triggers: Query<
        (Entity, &Transform, &Object, &Trigger),
        (
            With<XPosActivate>,
            Without<TriggerInProgress>,
            Without<TriggerActivated>,
        ),
    >,
    executing_triggers: ResMut<ExecutingTriggers>,
    groups: Res<Groups>,
    camera_transforms: Query<&Transform, (With<Camera2d>, Without<Object>, Without<XPosActivate>)>,
) {
    let player_x = if let Ok(transform) = camera_transforms.get_single() {
        transform.translation.x
    } else {
        return;
    };
    let commands_mutex = Arc::new(Mutex::new(commands));
    let triggers_mutex = Arc::new(Mutex::new(executing_triggers));
    triggers
        .par_iter()
        .for_each(|(entity, transform, object, trigger)| {
            if transform.translation.x > player_x {
                return;
            }
            for group_id in &object.groups {
                if let Some(group) = groups.0.get(group_id) {
                    if !group.activated {
                        return;
                    }
                }
            }
            if let Ok(mut executing_triggers) = triggers_mutex.lock() {
                let executing_triggers = executing_triggers
                    .0
                    .entry(trigger.0.get_target_group())
                    .or_default();
                if trigger.0.type_id() == TypeId::of::<RotateTrigger>() {
                    executing_triggers.retain(|t| t.type_id() != TypeId::of::<RotateTrigger>());
                }
                executing_triggers.push((entity, trigger.0.clone()));
            }
            if let Ok(mut commands) = commands_mutex.lock() {
                commands.entity(entity).insert(TriggerInProgress);
            }
        });
}

pub(crate) fn execute_triggers(world: &mut World) {
    world.resource_scope(|world, mut executing_triggers: Mut<ExecutingTriggers>| {
        for (_, triggers) in executing_triggers.0.iter_mut() {
            triggers.retain_mut(|(entity, trigger)| {
                trigger.execute(world);
                if trigger.done_executing() {
                    if let Some(mut entity) = world.get_entity_mut(*entity) {
                        entity
                            .remove::<TriggerInProgress>()
                            .insert(TriggerActivated);
                    }
                    false
                } else {
                    true
                }
            });
        }
    });
}

pub(crate) fn setup_trigger(
    commands: &mut Commands,
    entity: Entity,
    object_id: &u32,
    object_data: &HashMap<&[u8], &[u8]>,
) -> Result<(), anyhow::Error> {
    let mut entity = commands.entity(entity);
    match object_id {
        899 => {
            let mut trigger = ColorTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
            }
            if let Some(target_channel) = object_data.get(b"23".as_ref()) {
                trigger.target_channel = std::str::from_utf8(target_channel)?.parse()?;
            } else {
                trigger.target_channel = 1;
            }
            if let Some(r) = object_data.get(b"7".as_ref()) {
                trigger
                    .target_color
                    .set_r(std::str::from_utf8(r)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(g) = object_data.get(b"8".as_ref()) {
                trigger
                    .target_color
                    .set_g(std::str::from_utf8(g)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(b) = object_data.get(b"9".as_ref()) {
                trigger
                    .target_color
                    .set_b(std::str::from_utf8(b)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(opacity) = object_data.get(b"35".as_ref()) {
                trigger
                    .target_color
                    .set_a(std::str::from_utf8(opacity)?.parse()?);
            }
            if let Some(blending) = object_data.get(b"17".as_ref()) {
                trigger.target_blending = u8_to_bool(blending);
            }
            entity.insert(Trigger(Box::new(trigger)));
        }
        901 => {
            let mut trigger = MoveTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
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
                trigger.x_offset = std::str::from_utf8(x_offset)?.parse()?;
            }
            if let Some(y_offset) = object_data.get(b"29".as_ref()) {
                trigger.y_offset = std::str::from_utf8(y_offset)?.parse()?;
            }
            if let Some(lock_x) = object_data.get(b"58".as_ref()) {
                trigger.lock_x = u8_to_bool(lock_x);
            }
            if let Some(lock_y) = object_data.get(b"59".as_ref()) {
                trigger.lock_y = u8_to_bool(lock_y);
            }
            entity.insert(Trigger(Box::new(trigger)));
        }
        1007 => {
            let mut trigger = AlphaTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
            }
            if let Some(target_group) = object_data.get(b"51".as_ref()) {
                trigger.target_group = std::str::from_utf8(target_group)?.parse()?;
            }
            if let Some(target_opacity) = object_data.get(b"35".as_ref()) {
                trigger.target_opacity = std::str::from_utf8(target_opacity)?.parse()?;
            } else {
                trigger.target_opacity = 1.;
            }
            entity.insert(Trigger(Box::new(trigger)));
        }
        1049 => {
            let mut trigger = ToggleTrigger::default();
            if let Some(target_group) = object_data.get(b"51".as_ref()) {
                trigger.target_group = std::str::from_utf8(target_group)?.parse()?;
            }
            if let Some(activate) = object_data.get(b"56".as_ref()) {
                trigger.activate = u8_to_bool(activate);
            }
            entity.insert(Trigger(Box::new(trigger)));
        }
        1346 => {
            let mut trigger = RotateTrigger::default();
            if let Some(duration) = object_data.get(b"10".as_ref()) {
                trigger.duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
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
                trigger.degrees = std::str::from_utf8(degrees)?.parse()?;
            }
            if let Some(times360) = object_data.get(b"69".as_ref()) {
                trigger.times360 = std::str::from_utf8(times360)?.parse()?;
            }
            entity.insert(Trigger(Box::new(trigger)));
        }
        _ => (),
    }

    let touch_triggered = object_data
        .get(b"28".as_ref())
        .map(|b| u8_to_bool(b))
        .unwrap_or_default();
    let spawn_triggered = object_data
        .get(b"62".as_ref())
        .map(|b| u8_to_bool(b))
        .unwrap_or_default();
    if touch_triggered {
        entity.insert(TouchActivate);
    } else if spawn_triggered {
        entity.insert(SpawnActivate);
    } else {
        entity.insert(XPosActivate);
    }
    Ok(())
}