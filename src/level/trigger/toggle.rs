use crate::level::trigger::{TriggerCompleted, TriggerFunction};
use crate::states::play::{ColorChannels, ObjectColor};
use bevy::prelude::{Entity, Events, Mut, Text, TextureAtlasSprite, Transform, Visibility};
use std::time::Duration;

pub(crate) struct ToggleTrigger {
    pub(crate) target_group: u64,
    pub(crate) activate: bool,
}

impl TriggerFunction for ToggleTrigger {
    fn request_entities(&self) -> Vec<u64> {
        vec![self.target_group]
    }

    fn reset(&mut self) {}

    fn tick(
        &mut self,
        delta: Duration,
        entity: Entity,
        events: &mut Mut<Events<TriggerCompleted>>,
    ) {
        events.send(TriggerCompleted(entity))
    }

    fn execute(
        &mut self,
        group: &u64,
        transform: &mut Mut<Transform>,
        color: Option<&mut Mut<ObjectColor>>,
        visibility: Option<&mut Mut<Visibility>>,
        channels: &mut Mut<ColorChannels>,
    ) {
        if let Some(visibility) = visibility {
            visibility.is_visible = self.activate;
        }
    }
}
