use bevy::prelude::{Mut, World};

use crate::level::group::{GlobalGroup, GlobalGroups};
use crate::level::trigger::TriggerFunction;

#[derive(Default)]
pub(crate) struct ToggleTrigger {
    pub(crate) target_group: u64,
    pub(crate) activate: bool,
}

impl TriggerFunction for ToggleTrigger {
    fn execute(&self, world: &mut World, _: f32, _: f32) {
        world.resource_scope(|world, global_groups: Mut<GlobalGroups>| {
            let mut group_query = world.query::<&mut GlobalGroup>();

            let Some(group_entity) = global_groups.0.get(&self.target_group) else {
                return;
            };

            let Ok(mut global_group) = group_query.get_mut(world, *group_entity) else {
                return;
            };

            global_group.activated = self.activate;
        });
    }

    fn duration(&self) -> f32 {
        0.
    }

    fn exclusive(&self) -> bool {
        false
    }
}
