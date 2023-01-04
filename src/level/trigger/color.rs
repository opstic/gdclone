use crate::level::color::ColorChannel;
use crate::level::trigger::{TriggerCompleted, TriggerDuration, TriggerFunction};
use crate::loaders::gdlevel::GDColorChannel::BaseColor;
use crate::loaders::gdlevel::{GDBaseColor, GDColorChannel};
use crate::states::play::{ColorChannels, ObjectColor};
use crate::utils::lerp;
use bevy::log::info;
use bevy::prelude::{Color, Entity, Events, Mut, Transform, Visibility};
use std::sync::mpsc::channel;
use std::time::Duration;

pub(crate) struct ColorTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) target_channel: u64,
    pub(crate) target_r: u8,
    pub(crate) target_g: u8,
    pub(crate) target_b: u8,
    pub(crate) target_opacity: f32,
    pub(crate) target_blending: bool,
}

impl TriggerFunction for ColorTrigger {
    fn request_entities(&self) -> Vec<u64> {
        vec![u64::MAX]
    }

    fn reset(&mut self) {
        self.duration.reset()
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
        let progress = self.duration.fraction_progress();
        let entry = channels.colors.entry(self.target_channel);
        let channel = entry.or_default();
        let out;
        if let BaseColor(color) = channel {
            color.index = self.target_channel;
            color.r = lerp(&color.original_r, &self.target_r, &progress);
            color.g = lerp(&color.original_g, &self.target_g, &progress);
            color.b = lerp(&color.original_b, &self.target_b, &progress);
            color.opacity = lerp(&color.original_opacity, &self.target_opacity, &progress);
            color.blending = self.target_blending;
            if progress == 1. {
                color.original_r = color.r;
                color.original_g = color.g;
                color.original_b = color.b;
                color.original_opacity = color.opacity;
            }
            out = color.clone();
        } else {
            info!("hey what???? {}", self.target_channel);
            return;
        }
        *channel = BaseColor(out);
    }
}
