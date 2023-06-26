use std::any::TypeId;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::{
    Camera2d, Color, Commands, Component, Entity, Mut, Query, Res, ResMut, Resource, SystemSet,
    Transform, With, Without, World,
};
use bevy::utils::{hashbrown, HashMap, PassHash};
use dyn_clone::DynClone;

use crate::level::color::{ColorMod, Hsv};
use crate::level::easing::Easing;
use crate::level::object::Object;
use crate::level::trigger::alpha::AlphaTrigger;
use crate::level::trigger::color::ColorTrigger;
use crate::level::trigger::pulse::PulseTrigger;
use crate::level::trigger::r#move::MoveTrigger;
use crate::level::trigger::rotate::RotateTrigger;
use crate::level::trigger::toggle::ToggleTrigger;
use crate::level::Groups;
use crate::utils::{u8_to_bool, PassHashMap};

pub(crate) mod alpha;
pub(crate) mod color;
pub(crate) mod r#move;
pub(crate) mod pulse;
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
    pub(crate) HashMap<u64, Vec<(Entity, Box<dyn TriggerFunction>, f32)>>,
);

#[derive(Component)]
pub(crate) struct Trigger(pub(crate) Box<dyn TriggerFunction>);

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub(crate) enum TriggerSystems {
    ActivateTriggers,
    ExecuteTriggers,
}

#[derive(Clone, Debug, Default)]
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

pub(crate) trait TriggerFunction: Send + Sync + DynClone + Debug + 'static {
    fn execute(&mut self, world: &mut World);

    fn get_target_group(&self) -> u64;

    fn done_executing(&self) -> bool;

    fn exclusive(&self) -> bool;

    fn concrete_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

dyn_clone::clone_trait_object!(TriggerFunction);

pub(crate) fn activate_xpos_triggers(
    mut commands: Commands,
    triggers: Query<
        (Entity, &Transform, &Object, &Trigger),
        (
            With<XPosActivate>,
            Without<TriggerInProgress>,
            Without<TriggerActivated>,
        ),
    >,
    mut executing_triggers: ResMut<ExecutingTriggers>,
    groups: Res<Groups>,
    camera_transforms: Query<&Transform, (With<Camera2d>, Without<Object>, Without<XPosActivate>)>,
) {
    let player_x = if let Ok(transform) = camera_transforms.get_single() {
        transform.translation.x
    } else {
        return;
    };
    let mut triggers_to_be_executed: PassHashMap<Vec<(Entity, Box<dyn TriggerFunction>, f32)>> =
        hashbrown::HashMap::with_hasher(PassHash);
    let triggers_mutex = Arc::new(Mutex::new(&mut triggers_to_be_executed));
    triggers
        .par_iter()
        .for_each(|(entity, transform, object, trigger)| {
            if transform.translation.x > player_x {
                return;
            }
            for group_id in &object.groups {
                if let Some((group, _, _)) = groups.0.get(group_id) {
                    if !group.activated {
                        return;
                    }
                }
            }
            if let Ok(mut triggers_to_be_executed) = triggers_mutex.lock() {
                let triggers_to_be_executed = triggers_to_be_executed
                    .entry(trigger.0.get_target_group())
                    .or_default();
                triggers_to_be_executed.push((entity, trigger.0.clone(), transform.translation.x));
            }
        });
    let triggers_to_be_executed = Arc::try_unwrap(triggers_mutex)
        .unwrap()
        .into_inner()
        .unwrap();
    for (group, mut triggers) in std::mem::take(triggers_to_be_executed) {
        triggers.sort_unstable_by(|(_, _, x_a), (_, _, x_b)| x_a.partial_cmp(x_b).unwrap());
        for (entity, _, _) in &triggers {
            commands.entity(*entity).insert(TriggerInProgress);
        }
        let triggers_entry = executing_triggers.0.entry(group).or_default();
        triggers_entry.extend(triggers);
    }
}

pub(crate) fn execute_triggers(world: &mut World) {
    world.resource_scope(|world, mut executing_triggers: Mut<ExecutingTriggers>| {
        for (_, triggers) in executing_triggers.0.iter_mut() {
            let mut override_triggers = HashMap::new();
            triggers.retain_mut(|(entity, trigger, x_pos)| {
                trigger.execute(world);
                if trigger.exclusive() {
                    override_triggers.insert((*trigger).concrete_type_id(), *x_pos);
                }
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
            for (override_trigger_type, override_x_pos) in override_triggers {
                triggers.retain(|(entity, trigger, x_pos)| {
                    if (*trigger).concrete_type_id() == override_trigger_type
                        && x_pos < &override_x_pos
                    {
                        if let Some(mut entity) = world.get_entity_mut(*entity) {
                            entity
                                .remove::<TriggerInProgress>()
                                .insert(TriggerActivated);
                        }
                        false
                    } else {
                        true
                    }
                })
            }
        }
    });
}

pub(crate) fn setup_trigger(
    commands: &mut Commands,
    entity: Entity,
    object_id: &u64,
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
            if let Some(copied_hsv) = object_data.get(b"49".as_ref()) {
                trigger.copied_hsv = Hsv::parse(copied_hsv)?;
            }
            if let Some(copied_channel) = object_data.get(b"50".as_ref()) {
                trigger.copied_channel = std::str::from_utf8(copied_channel)?.parse()?;
            } else {
                trigger.copied_channel = 0;
            }
            if let Some(copy_opacity) = object_data.get(b"60".as_ref()) {
                trigger.copy_opacity = u8_to_bool(copy_opacity);
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
        1006 => {
            let mut trigger = PulseTrigger::default();
            if let Some(fade_in_duration) = object_data.get(b"45".as_ref()) {
                trigger.fade_in_duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(fade_in_duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
            }
            if let Some(hold_duration) = object_data.get(b"46".as_ref()) {
                trigger.hold_duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(hold_duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
            }
            if let Some(fade_out_duration) = object_data.get(b"47".as_ref()) {
                trigger.fade_out_duration = TriggerDuration::new(
                    Duration::try_from_secs_f32(std::str::from_utf8(fade_out_duration)?.parse()?)
                        .unwrap_or(Duration::ZERO),
                )
            }
            let mut mod_mode = false;
            if let Some(hsv_mode) = object_data.get(b"48".as_ref()) {
                mod_mode = u8_to_bool(hsv_mode);
            }
            if mod_mode {
                let mut hsv = Hsv::default();
                let mut copied_color_id = 0;
                if let Some(targer_hsv) = object_data.get(b"49".as_ref()) {
                    hsv = Hsv::parse(targer_hsv)?;
                }
                if let Some(color_id) = object_data.get(b"50".as_ref()) {
                    copied_color_id = std::str::from_utf8(color_id)?.parse()?;
                }
                // Ignore trigger when the copied id is 0
                if copied_color_id == 0 {
                    return Ok(());
                }
                trigger.color_mod = ColorMod::Hsv(copied_color_id, hsv, 1.);
            } else {
                let mut color = Color::WHITE;
                if let Some(r) = object_data.get(b"7".as_ref()) {
                    color.set_r(std::str::from_utf8(r)?.parse::<u8>()? as f32 / u8::MAX as f32);
                }
                if let Some(g) = object_data.get(b"8".as_ref()) {
                    color.set_g(std::str::from_utf8(g)?.parse::<u8>()? as f32 / u8::MAX as f32);
                }
                if let Some(b) = object_data.get(b"9".as_ref()) {
                    color.set_b(std::str::from_utf8(b)?.parse::<u8>()? as f32 / u8::MAX as f32);
                }
                trigger.color_mod = ColorMod::Color(color, 1.);
            }
            if let Some(target_id) = object_data.get(b"51".as_ref()) {
                trigger.target_id = std::str::from_utf8(target_id)?.parse()?;
            }
            if let Some(target_color_channel) = object_data.get(b"52".as_ref()) {
                trigger.target_group = u8_to_bool(target_color_channel);
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
        .get(b"11".as_ref())
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
