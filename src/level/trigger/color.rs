use crate::level::color::{BaseColor, ColorChannel, ColorChannels, CopyColor, Hsv};
use crate::level::trigger::{TriggerDuration, TriggerFunction};
use crate::utils::lerp_color;
use bevy::ecs::system::SystemState;
use bevy::prelude::{Color, Res, ResMut, World};
use bevy::time::Time;

#[derive(Clone, Default)]
pub(crate) struct ColorTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) target_channel: u32,
    pub(crate) copied_channel: u32,
    pub(crate) target_color: Color,
    pub(crate) copied_hsv: Hsv,
    pub(crate) copy_opacity: bool,
    pub(crate) target_blending: bool,
    pub(crate) original_color: Color,
    pub(crate) not_initial: bool,
}

impl TriggerFunction for ColorTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<(Res<Time>, ResMut<ColorChannels>)> =
            SystemState::new(world);
        let (time, mut color_channels) = system_state.get_mut(world);
        self.duration.tick(time.delta());
        if !self.not_initial {
            let (channel_color, _) = color_channels.get_color(&self.target_channel);
            self.original_color = channel_color;
        }
        let (copied_channel_color, _) = color_channels.get_color(&self.copied_channel);
        let channel = color_channels.0.entry(self.target_channel).or_default();
        if self.duration.completed() || self.duration.duration.is_zero() {
            if self.copied_channel != 0 {
                *channel = ColorChannel::CopyColor(CopyColor {
                    copied_index: self.copied_channel,
                    copy_opacity: self.copy_opacity,
                    opacity: self.target_color.a(),
                    blending: self.target_blending,
                    hsv: self.copied_hsv.clone(),
                })
            } else {
                *channel = ColorChannel::BaseColor(BaseColor {
                    color: self.target_color,
                    blending: self.target_blending,
                })
            }
            return;
        }
        let progress = self.duration.fraction_progress();
        *channel = if self.copied_channel != 0 {
            ColorChannel::BaseColor(BaseColor {
                color: self.copied_hsv.apply(lerp_color(
                    &self.original_color,
                    &copied_channel_color,
                    &progress,
                )),
                blending: self.target_blending,
            })
        } else {
            ColorChannel::BaseColor(BaseColor {
                color: lerp_color(&self.original_color, &self.target_color, &progress),
                blending: self.target_blending,
            })
        };
        self.not_initial = true;
    }

    fn get_target_group(&self) -> u32 {
        self.target_channel
    }

    fn done_executing(&self) -> bool {
        self.duration.completed()
    }
}
