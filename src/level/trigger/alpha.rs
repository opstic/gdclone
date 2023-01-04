use crate::level::easing::Easing;
use crate::level::trigger::{TriggerCompleted, TriggerDuration, TriggerFunction};
use crate::states::play::{ColorChannels, Groups, LevelObject, ObjectColor};
use bevy::log::info;
use bevy::prelude::{
    Commands, Entity, Events, Mut, Query, Res, Transform, Visibility, With, World,
};
use bevy::sprite::TextureAtlasSprite;
use bevy::text::Text;
use bevy::utils::HashMap;
use interpolation::lerp;
use std::time::Duration;

pub(crate) struct AlphaTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) target_group: u64,
    pub(crate) target_opacity: f32,
}

impl TriggerFunction for AlphaTrigger {
    fn request_entities(&self) -> Vec<u64> {
        vec![self.target_group]
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
    }

    fn execute(
        &mut self,
        group: &u64,
        transform: &mut Mut<Transform>,
        color: Option<&mut Mut<ObjectColor>>,
        visibility: Option<&mut Mut<Visibility>>,
        channels: &mut Mut<ColorChannels>,
    ) {
        if let Some(color) = color {
            color.2 = lerp(
                &color.3,
                &self.target_opacity,
                &self.duration.fraction_progress(),
            );
            if self.duration.fraction_progress() == 1. {
                color.3 = color.2;
            }
        }
    }
}
