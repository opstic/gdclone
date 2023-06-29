use bevy::ecs::system::SystemState;
use bevy::prelude::{Res, ResMut, World};
use bevy::time::Time;

use crate::level::{
    trigger::{TriggerDuration, TriggerFunction},
    Groups,
};
use crate::utils::lerp;

#[derive(Clone, Debug, Default)]
pub(crate) struct AlphaTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) target_group: u64,
    pub(crate) target_opacity: f32,
    pub(crate) original_opacity: f32,
    pub(crate) not_initial: bool,
}

impl TriggerFunction for AlphaTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<(Res<Time>, ResMut<Groups>)> = SystemState::new(world);
        let (time, mut groups) = system_state.get_mut(world);
        self.duration.tick(time.delta());
        let fractional_progress = self.duration.fraction_progress();
        if let Some((group, _, _)) = groups.0.get_mut(&self.target_group) {
            if !self.not_initial {
                self.original_opacity = group.opacity;
            }
            if self.duration.completed() || self.duration.duration.is_zero() {
                group.opacity = self.target_opacity;
            } else {
                group.opacity = lerp(
                    &self.original_opacity,
                    &self.target_opacity,
                    &fractional_progress,
                );
            }
        }
        self.not_initial = true;
    }

    fn get_target_group(&self) -> u64 {
        self.target_group
    }

    fn done_executing(&self) -> bool {
        self.duration.completed() || self.duration.duration.is_zero()
    }

    fn exclusive(&self) -> bool {
        true
    }
}
