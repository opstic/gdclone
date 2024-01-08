use std::any::Any;

use bevy::ecs::system::SystemState;
use bevy::prelude::{Color, Mut, Query, World};

use crate::level::color::{
    ColorChannelCalculated, GlobalColorChannel, GlobalColorChannels, HsvMod,
};
use crate::level::trigger::TriggerFunction;
use crate::utils::{lerp_color, lerp_start_color};

#[derive(Clone, Debug, Default)]
pub(crate) struct ColorTrigger {
    pub(crate) duration: f32,
    pub(crate) target_channel: u64,
    pub(crate) copied_channel: u64,
    pub(crate) target_color: Color,
    pub(crate) copied_hsv: Option<HsvMod>,
    pub(crate) copy_opacity: bool,
    pub(crate) target_blending: bool,
}

type ColorTriggerSystemParam = Query<
    'static,
    'static,
    (
        &'static mut GlobalColorChannel,
        &'static ColorChannelCalculated,
    ),
>;

impl TriggerFunction for ColorTrigger {
    fn execute(
        &self,
        world: &mut World,
        system_state: &mut Box<dyn Any + Send + Sync>,
        previous_progress: f32,
        progress: f32,
    ) {
        let (target_entity, parent_data) =
            world.resource_scope(|world, global_color_channels: Mut<GlobalColorChannels>| {
                let target_entity = if let Some(target_entity) =
                    global_color_channels.0.get(&self.target_channel)
                {
                    *target_entity
                } else {
                    let entity = world
                        .spawn((
                            GlobalColorChannel::default(),
                            ColorChannelCalculated::default(),
                        ))
                        .id();
                    global_color_channels.0.insert(self.target_channel, entity);
                    entity
                };

                let parent_entity = if self.copied_channel != 0 {
                    let parent_entity = if let Some(target_entity) =
                        global_color_channels.0.get(&self.copied_channel)
                    {
                        *target_entity
                    } else {
                        let entity = world
                            .spawn((
                                GlobalColorChannel::default(),
                                ColorChannelCalculated::default(),
                            ))
                            .id();
                        global_color_channels.0.insert(self.copied_channel, entity);
                        entity
                    };
                    let copied_color = world
                        .entity(parent_entity)
                        .get::<ColorChannelCalculated>()
                        .unwrap()
                        .color;
                    Some((parent_entity, copied_color))
                } else {
                    None
                };

                (target_entity, parent_entity)
            });

        let system_state: &mut SystemState<ColorTriggerSystemParam> =
            system_state.downcast_mut().unwrap();

        let mut color_channel_query = system_state.get_mut(world);

        let Ok((mut color_channel, calculated)) = color_channel_query.get_mut(target_entity) else {
            return;
        };

        if progress == 1. {
            if self.copied_channel != 0 {
                *color_channel = GlobalColorChannel::Copy {
                    copied_index: self.copied_channel,
                    copy_opacity: self.copy_opacity,
                    opacity: self.target_color.a(),
                    blending: self.target_blending,
                    hsv: self.copied_hsv,
                }
            } else {
                *color_channel = GlobalColorChannel::Base {
                    color: self.target_color,
                    blending: self.target_blending,
                }
            }

            return;
        }

        let target_color = if let Some((_, target_color)) = parent_data {
            let mut target_color = target_color;
            if let Some(hsv) = self.copied_hsv {
                hsv.apply(&mut target_color);
            }
            target_color
        } else {
            self.target_color
        };

        let original_color = lerp_start_color(&calculated.color, &target_color, previous_progress);

        *color_channel = GlobalColorChannel::Base {
            color: lerp_color(&original_color, &target_color, progress),
            blending: self.target_blending,
        };
    }

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync> {
        Box::new(SystemState::<ColorTriggerSystemParam>::new(world))
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }
}
