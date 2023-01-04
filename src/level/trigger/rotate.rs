use crate::level::easing::Easing;
use crate::level::trigger::{TriggerCompleted, TriggerDuration, TriggerFunction};
use crate::states::play::{ColorChannels, ObjectColor};
use bevy::math::{Quat, Vec3};
use bevy::prelude::{Entity, Events, Mut, Text, TextureAtlasSprite, Transform, Visibility};
use std::time::Duration;

pub(crate) struct RotateTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) center_group: u64,
    pub(crate) degrees: i32,
    pub(crate) times360: i32,
    pub(crate) amount: f32,
    pub(crate) previous_amount: f32,
    pub(crate) center_translation: Vec3,
}

impl TriggerFunction for RotateTrigger {
    fn request_entities(&self) -> Vec<u64> {
        vec![self.center_group, self.target_group]
    }

    fn reset(&mut self) {
        self.duration.reset();
    }

    fn tick(
        &mut self,
        delta: Duration,
        entity: Entity,
        events: &mut Mut<Events<TriggerCompleted>>,
    ) {
        self.duration.tick(delta);
        if self.duration.completed() {
            events.send(TriggerCompleted(entity));
        }
        self.amount = self.easing.sample(self.duration.fraction_progress()) - self.previous_amount;
        self.previous_amount = self.easing.sample(self.duration.fraction_progress());
    }

    fn execute(
        &mut self,
        group: &u64,
        transform: &mut Mut<Transform>,
        color: Option<&mut Mut<ObjectColor>>,
        visibility: Option<&mut Mut<Visibility>>,
        channels: &mut Mut<ColorChannels>,
    ) {
        if group == &self.center_group {
            self.center_translation = transform.translation;
        } else {
            if self.center_group == 0 {
                transform.rotate(Quat::from_rotation_z(
                    -((360 * self.times360 + self.degrees) as f32 * self.amount).to_radians(),
                ));
            } else {
                transform.rotate_around(
                    self.center_translation,
                    Quat::from_rotation_z(
                        -((360 * self.times360 + self.degrees) as f32 * self.amount).to_radians(),
                    ),
                );
            }
        }
    }
}
