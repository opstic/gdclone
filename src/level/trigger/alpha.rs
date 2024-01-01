use bevy::prelude::{Mut, World};

use crate::level::group::{GlobalGroup, GlobalGroups};
use crate::level::trigger::TriggerFunction;
use crate::utils::{lerp, lerp_start};

#[derive(Default)]
pub(crate) struct AlphaTrigger {
    pub(crate) duration: f32,
    pub(crate) target_group: u64,
    pub(crate) target_opacity: f32,
}

impl TriggerFunction for AlphaTrigger {
    fn execute(&self, world: &mut World, previous_progress: f32, progress: f32) {
        world.resource_scope(|world, global_groups: Mut<GlobalGroups>| {
            let mut group_query = world.query::<&mut GlobalGroup>();

            let Some(group_entity) = global_groups.0.get(&self.target_group) else {
                return;
            };

            let Ok(mut global_group) = group_query.get_mut(world, *group_entity) else {
                return;
            };

            let original_opacity =
                lerp_start(global_group.opacity, self.target_opacity, previous_progress);
            global_group.opacity = lerp(original_opacity, self.target_opacity, progress);
        });
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }
}
