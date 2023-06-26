use bevy::ecs::system::SystemState;
use bevy::prelude::{Res, ResMut, World};
use bevy::time::Time;

use crate::level::color::{ColorChannels, ColorMod};
use crate::level::trigger::{TriggerDuration, TriggerFunction};
use crate::level::Groups;

#[derive(Clone, Debug, Default)]
pub(crate) struct PulseTrigger {
    pub(crate) fade_in_duration: TriggerDuration,
    pub(crate) hold_duration: TriggerDuration,
    pub(crate) fade_out_duration: TriggerDuration,
    pub(crate) target_id: u64,
    pub(crate) target_group: bool,
    pub(crate) color_mod: ColorMod,
    pub(crate) base_only: bool,
    pub(crate) detail_only: bool,
    pub(crate) exclusive: bool,
}

impl TriggerFunction for PulseTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<(Res<Time>, ResMut<Groups>, ResMut<ColorChannels>)> =
            SystemState::new(world);
        let (time, mut groups, mut color_channels) = system_state.get_mut(world);

        let mut color_mod = self.color_mod;

        if !self.fade_in_duration.completed() && !self.fade_in_duration.duration.is_zero() {
            self.fade_in_duration.tick(time.delta());
            match &mut color_mod {
                ColorMod::Color(_, progress) => {
                    *progress = self.fade_in_duration.fraction_progress()
                }
                ColorMod::Hsv(_, _, progress) => {
                    *progress = self.fade_in_duration.fraction_progress()
                }
            }
        } else if !self.hold_duration.completed() && !self.hold_duration.duration.is_zero() {
            self.hold_duration.tick(time.delta());
            match &mut color_mod {
                ColorMod::Color(_, progress) => *progress = 1.,
                ColorMod::Hsv(_, _, progress) => *progress = 1.,
            }
        } else if !self.fade_out_duration.completed() && !self.fade_out_duration.duration.is_zero()
        {
            self.fade_out_duration.tick(time.delta());
            match &mut color_mod {
                ColorMod::Color(_, progress) => {
                    *progress = 1. - self.fade_out_duration.fraction_progress()
                }
                ColorMod::Hsv(_, _, progress) => {
                    *progress = 1. - self.fade_out_duration.fraction_progress()
                }
            }
        }

        let color_mod = if self.done_executing() {
            None
        } else {
            Some(color_mod)
        };
        if !self.target_group {
            color_channels.0.entry(self.target_id).or_default().1 = color_mod;
        } else if let Some((_, ref mut base_color_mod, ref mut detail_color_mod)) =
            groups.0.get_mut(&self.target_id)
        {
            if !self.detail_only {
                *base_color_mod = color_mod;
            }
            if !self.base_only {
                *detail_color_mod = color_mod;
            }
        }
    }

    fn get_target_group(&self) -> u64 {
        if self.target_group {
            self.target_id + (u64::MAX / 2)
        } else {
            self.target_id
        }
    }

    fn done_executing(&self) -> bool {
        (self.fade_in_duration.completed() || self.fade_in_duration.duration.is_zero())
            && (self.hold_duration.completed() || self.hold_duration.duration.is_zero())
            && (self.fade_out_duration.completed() || self.fade_out_duration.duration.is_zero())
    }

    fn exclusive(&self) -> bool {
        self.exclusive
    }
}
