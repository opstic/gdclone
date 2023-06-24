use crate::level::trigger::TriggerFunction;
use crate::level::Groups;
use bevy::ecs::system::SystemState;
use bevy::prelude::{ResMut, World};

#[derive(Clone, Debug, Default)]
pub(crate) struct ToggleTrigger {
    pub(crate) target_group: u64,
    pub(crate) activate: bool,
}

impl TriggerFunction for ToggleTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<ResMut<Groups>> = SystemState::new(world);
        let mut groups = system_state.get_mut(world);
        if let Some(group) = groups.0.get_mut(&self.target_group) {
            group.activated = self.activate;
        }
    }

    fn get_target_group(&self) -> u64 {
        self.target_group
    }

    fn done_executing(&self) -> bool {
        true
    }
}
