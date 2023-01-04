use crate::states::play::{ColorChannels, Groups, LevelObject, ObjectColor, Player};
use bevy::log::info;
use bevy::math;
use bevy::prelude::{
    Commands, Component, Entity, EventReader, Events, Mut, Query, Res, ResMut, SystemLabel,
    Transform, Visibility, With, Without,
};
use bevy::sprite::TextureAtlasSprite;
use bevy::text::Text;
use bevy::time::Time;
use interpolation::{Ease, EaseFunction};
use serde::Deserialize;
use std::f32::consts;
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
pub(crate) struct TriggerInProgress;

#[derive(Component)]
pub(crate) struct TriggerActivated;

#[derive(Component)]
pub(crate) struct Trigger(pub Box<dyn TriggerFunction>);

#[derive(Copy, Clone)]
pub(crate) struct TriggerCompleted(pub(crate) Entity);

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub(crate) enum TriggerSystems {
    ActivateTriggers,
    TickTriggers,
    DeactivateTriggers,
}

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

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
    }
}

pub(crate) trait TriggerFunction: Send + Sync {
    fn request_entities(&self) -> Vec<u64>;

    fn reset(&mut self);

    fn tick(&mut self, delta: Duration, entity: Entity, events: &mut Mut<Events<TriggerCompleted>>);

    fn execute(
        &mut self,
        group: &u64,
        transform: &mut Mut<Transform>,
        color: Option<&mut Mut<ObjectColor>>,
        visibility: Option<&mut Mut<Visibility>>,
        channels: &mut Mut<ColorChannels>,
    );
}

pub(crate) fn tick_triggers(
    mut triggers: Query<
        (Entity, &mut Trigger),
        (With<TriggerInProgress>, Without<TriggerActivated>),
    >,
    mut objects: Query<
        (&mut Transform, &mut ObjectColor, &mut Visibility),
        (With<LevelObject>, Without<Trigger>),
    >,
    mut player: Query<&mut Transform, (With<Player>, Without<LevelObject>)>,
    time: Res<Time>,
    groups: Res<Groups>,
    mut channels: ResMut<ColorChannels>,
    mut trigger_completed_events: ResMut<Events<TriggerCompleted>>,
) {
    let mut trigger_completed_events: Mut<Events<TriggerCompleted>> =
        trigger_completed_events.into();
    let mut channels: Mut<ColorChannels> = channels.into();
    for (entity, mut trigger) in triggers.iter_mut() {
        trigger
            .0
            .tick(time.delta(), entity, &mut trigger_completed_events);
        for index in trigger.0.request_entities().iter() {
            if index == &u64::MAX {
                for mut transform in player.iter_mut() {
                    trigger
                        .0
                        .execute(index, &mut transform, None, None, &mut channels);
                }
            } else if let Some(group) = groups.groups.get(index) {
                for object in group {
                    if let Ok((mut transform, mut color, mut visibility)) = objects.get_mut(*object)
                    {
                        trigger.0.execute(
                            index,
                            &mut transform,
                            Some(&mut color),
                            Some(&mut visibility),
                            &mut channels,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn finish_triggers(
    mut commands: Commands,
    mut triggers: Query<(&mut Trigger, Option<&MultiActivate>)>,
    mut completed_triggers: EventReader<TriggerCompleted>,
) {
    for completed_trigger in completed_triggers.iter() {
        let mut entity = commands.entity(completed_trigger.0);
        entity.remove::<TriggerInProgress>();
        if let Ok((mut trigger, multi_activate)) = triggers.get_mut(completed_trigger.0) {
            trigger.0.reset();
            if multi_activate.is_none() {
                entity.insert(TriggerActivated);
            }
        }
    }
}
