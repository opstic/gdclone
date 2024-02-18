use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::math::Vec3A;
use bevy::prelude::{Entity, Query, Res, World};

use crate::level::color::{
    ColorChannelCalculated, ColorMod, GlobalColorChannels, ObjectColorKind, Pulses,
};
use crate::level::group::GlobalGroups;
use crate::level::trigger::TriggerFunction;

#[derive(Clone, Debug, Default)]
pub(crate) struct PulseTrigger {
    pub(crate) fade_in_duration: f32,
    pub(crate) hold_duration: f32,
    pub(crate) fade_out_duration: f32,
    pub(crate) target_id: u64,
    pub(crate) target_is_group: bool,
    pub(crate) color_mod: ColorMod,
    pub(crate) copied_color_id: u64,
    pub(crate) base_only: bool,
    pub(crate) detail_only: bool,
    pub(crate) exclusive: bool,
}

type PulseTriggerSystemParam = (
    Res<'static, GlobalGroups>,
    Res<'static, GlobalColorChannels>,
    Query<'static, 'static, &'static mut Pulses>,
    Query<'static, 'static, &'static ColorChannelCalculated>,
);

impl TriggerFunction for PulseTrigger {
    fn execute(
        &self,
        world: &mut World,
        _: Entity,
        _: u32,
        system_state: &mut Box<dyn Any + Send + Sync>,
        _: f32,
        progress: f32,
    ) {
        if self.base_only && self.copied_color_id == 0 {
            return;
        }

        let system_state: &mut SystemState<PulseTriggerSystemParam> =
            &mut *system_state.downcast_mut().unwrap();

        let (global_groups, global_color_channels, mut pulse_query, color_channel_query) =
            system_state.get_mut(world);

        let total_time = self.duration();
        let fade_in = self.fade_in_duration / total_time;
        let hold = self.hold_duration / total_time;
        let fade_out = self.fade_out_duration / total_time;

        let target_entity = if self.target_is_group {
            global_groups.0[self.target_id as usize]
        } else {
            let Some(target_entity) = global_color_channels.0.get(&self.target_id) else {
                return;
            };
            *target_entity
        };

        let Ok(mut target_pulses) = pulse_query.get_mut(target_entity) else {
            return;
        };

        let final_mod = match self.color_mod {
            ColorMod::Color(_) => self.color_mod,
            ColorMod::Hsv(hsv) => {
                if self.copied_color_id == 0 {
                    self.color_mod
                } else if let Some(entity) = global_color_channels.0.get(&self.copied_color_id) {
                    if let Ok(calculated) = color_channel_query.get(*entity) {
                        let mut color = Vec3A::from(if self.copied_color_id == self.target_id {
                            calculated.pre_pulse_color
                        } else {
                            calculated.color
                        });
                        hsv.apply_rgb(&mut color);
                        ColorMod::Color(color)
                    } else {
                        self.color_mod
                    }
                } else {
                    ColorMod::Color(Vec3A::ONE)
                }
            }
        };

        let target_object_kind = if self.base_only {
            ObjectColorKind::Base
        } else if self.detail_only {
            ObjectColorKind::Detail
        } else {
            ObjectColorKind::None
        };

        if progress <= fade_in {
            target_pulses
                .pulses
                .push((progress / fade_in, final_mod, target_object_kind))
        } else if progress <= fade_in + hold {
            target_pulses.pulses.clear();
            target_pulses
                .pulses
                .push((1., final_mod, target_object_kind));
        } else if progress <= fade_in + hold + fade_out {
            target_pulses.pulses.push((
                1. - (progress - fade_in - hold) / fade_out,
                final_mod,
                target_object_kind,
            ))
        }
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<PulseTriggerSystemParam>::new(world))
    }

    fn target_id(&self) -> u64 {
        if self.target_is_group {
            (u64::MAX / 2) + self.target_id
        } else {
            self.target_id
        }
    }

    fn duration(&self) -> f32 {
        self.fade_in_duration + self.hold_duration + self.fade_out_duration
    }

    fn exclusive(&self) -> bool {
        self.exclusive
    }
}
