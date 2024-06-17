use bevy::ecs::system::SystemState;
use bevy::math::Vec4;
use bevy::prelude::{Entity, Query, Res, World};
use std::any::Any;
use std::ops::Range;

use crate::level::color::{
    ColorChannelCalculated, GlobalColorChannel, GlobalColorChannelKind, GlobalColorChannels, HsvMod,
};
use crate::level::trigger::TriggerFunction;
use crate::utils::lerp_start;

#[derive(Clone, Debug, Default)]
pub(crate) struct ColorTrigger {
    pub(crate) duration: f32,
    pub(crate) target_channel: u64,
    pub(crate) copied_channel: u64,
    pub(crate) target_color: Vec4,
    pub(crate) copied_hsv: Option<HsvMod>,
    pub(crate) copy_opacity: bool,
    pub(crate) target_blending: bool,
}

type ColorTriggerSystemParam = (
    Res<'static, GlobalColorChannels>,
    Query<
        'static,
        'static,
        (
            &'static mut GlobalColorChannel,
            &'static ColorChannelCalculated,
        ),
    >,
);

impl TriggerFunction for ColorTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
        _: Range<f32>,
    ) {
        let system_state: &mut SystemState<ColorTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let (global_color_channels, mut color_channel_query) = system_state.get_mut(world);

        let target_entity = *global_color_channels.0.get(&self.target_channel).unwrap();

        let copied_color = if self.copied_channel != 0 {
            global_color_channels
                .0
                .get(&self.copied_channel)
                .map(|entity| color_channel_query.get(*entity).unwrap().1.color)
        } else {
            None
        };

        let Ok((mut color_channel, calculated)) = color_channel_query.get_mut(target_entity) else {
            return;
        };

        if progress == 1. {
            if self.copied_channel != 0 {
                color_channel.kind = GlobalColorChannelKind::Copy {
                    copied_index: self.copied_channel,
                    copy_opacity: self.copy_opacity,
                    opacity: self.target_color[3],
                    blending: self.target_blending,
                    hsv: self.copied_hsv,
                }
            } else {
                color_channel.kind = GlobalColorChannelKind::Base {
                    color: self.target_color,
                    blending: self.target_blending,
                }
            }

            return;
        }

        let target_color = if let Some(copied_color) = copied_color {
            let mut target_color = copied_color;
            if let Some(hsv) = self.copied_hsv {
                hsv.apply_rgba(&mut target_color);
            }
            target_color
        } else {
            self.target_color
        };

        let original_color =
            lerp_start(calculated.pre_pulse_color, target_color, previous_progress);

        color_channel.kind = GlobalColorChannelKind::Base {
            color: original_color.lerp(target_color, progress),
            blending: self.target_blending,
        };
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<ColorTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        self.target_channel
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        true
    }

    fn post(&self) -> bool {
        false
    }
}
