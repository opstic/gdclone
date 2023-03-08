use crate::level::easing::Easing;
use crate::level::trigger::{TriggerCompleted, TriggerDuration, TriggerFunction};
use crate::states::play::{ColorChannels, Groups, LevelObject, ObjectColor};
use bevy::log::info;
use bevy::math::Vec2;
use bevy::prelude::{
    Commands, Entity, Events, Mut, Query, Res, Text, Transform, Visibility, With, World,
};
use bevy::sprite::TextureAtlasSprite;
use std::time::Duration;

pub(crate) struct MoveTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) easing: Easing,
    pub(crate) amount: f32,
    pub(crate) previous_amount: f32,
    pub(crate) target_group: u64,
    pub(crate) x_offset: f32,
    pub(crate) y_offset: f32,
    pub(crate) lock_x: bool,
    pub(crate) lock_y: bool,
    pub(crate) player_x: f32,
    pub(crate) player_y: f32,
    pub(crate) player_previous_x: f32,
    pub(crate) player_previous_y: f32,
}

impl TriggerFunction for MoveTrigger {
    fn request_entities(&self) -> Vec<u64> {
        if self.lock_x || self.lock_y {
            vec![u64::MAX, self.target_group]
        } else {
            vec![self.target_group]
        }
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
        if group == &u64::MAX {
            if self.player_previous_x == 0. && self.player_previous_y == 0. {
                self.player_previous_x = transform.translation.x;
                self.player_previous_y = transform.translation.y;
            } else {
                self.player_previous_x = self.player_x;
                self.player_previous_y = self.player_y;
            }
            self.player_x = transform.translation.x;
            self.player_y = transform.translation.y;
        } else {
            transform.translation += Vec2::new(
                self.x_offset * self.amount * 4.,
                self.y_offset * self.amount * 4.,
            )
            .extend(0.);
            if self.lock_x {
                transform.translation.x += self.player_x - self.player_previous_x;
            }
            if self.lock_y {
                transform.translation.y += self.player_y - self.player_previous_y;
            }
        }
    }
}
