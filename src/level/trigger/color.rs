use crate::level::color::BaseColor;
use crate::level::color::{ColorChannel, ColorChannels};
use crate::level::trigger::{TriggerDuration, TriggerFunction};
use crate::utils::lerp;
use bevy::ecs::system::SystemState;
use bevy::prelude::{Color, Res, ResMut, World};
use bevy::time::Time;

#[derive(Clone, Default)]
pub(crate) struct ColorTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) target_channel: u64,
    pub(crate) target_color: Color,
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
        let progress = self.duration.fraction_progress();
        let channel = color_channels.0.entry(self.target_channel).or_default();
        *channel = match channel {
            ColorChannel::BaseColor(color) => {
                let mut new_color = BaseColor::default();
                if !self.not_initial {
                    self.original_color = color.color;
                    new_color.blending = self.target_blending;
                }

                if self.duration.completed() {
                    new_color.color = self.target_color;
                    new_color.blending = self.target_blending;
                } else {
                    new_color.color.set_r(lerp(
                        &(self.original_color.r() as f64),
                        &(self.target_color.r() as f64),
                        &progress,
                    ) as f32);
                    new_color.color.set_g(lerp(
                        &(self.original_color.g() as f64),
                        &(self.target_color.g() as f64),
                        &progress,
                    ) as f32);
                    new_color.color.set_b(lerp(
                        &(self.original_color.b() as f64),
                        &(self.target_color.b() as f64),
                        &progress,
                    ) as f32);
                    new_color.color.set_a(lerp(
                        &(self.original_color.a() as f64),
                        &(self.target_color.a() as f64),
                        &progress,
                    ) as f32);
                }
                ColorChannel::BaseColor(new_color)
            }
            ColorChannel::CopyColor(_) => {
                return;
            }
        };
        self.not_initial = true;
    }

    fn get_target_group(&self) -> u64 {
        0
    }

    fn done_executing(&self) -> bool {
        self.duration.completed()
    }
}
