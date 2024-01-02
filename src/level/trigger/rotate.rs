use bevy::ecs::system::SystemState;
use bevy::math::{Quat, Vec3Swizzles};
use bevy::prelude::{Query, Res, Transform, With, Without, World};

use crate::level::easing::Easing;
use crate::level::group::{
    GlobalGroup, GlobalGroupDeltas, GlobalGroups, ObjectGroups, TransformDelta,
};
use crate::level::object::Object;
use crate::level::trigger::{Trigger, TriggerFunction};

#[derive(Clone, Debug, Default)]
pub(crate) struct RotateTrigger {
    pub(crate) duration: f32,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) center_group: u64,
    pub(crate) degrees: i32,
    pub(crate) times360: i32,
    pub(crate) lock_rotation: bool,
}

impl TriggerFunction for RotateTrigger {
    fn execute(&self, world: &mut World, previous_progress: f32, progress: f32) {
        let mut system_state: SystemState<(
            Res<GlobalGroups>,
            Query<&GlobalGroup>,
            Query<&mut GlobalGroupDeltas>,
            Query<(&Transform, &ObjectGroups), (With<Object>, Without<Trigger>)>,
        )> = SystemState::new(world);

        let (global_groups, group_query, mut group_delta_query, object_query) =
            system_state.get_mut(world);

        let Some(group_entity) = global_groups.0.get(&self.target_group) else {
            return;
        };

        let Ok(mut global_group_delta) = group_delta_query.get_mut(*group_entity) else {
            return;
        };

        // This is horrendously bad
        let center = if let Some(center_group_entity) = global_groups.0.get(&self.center_group) {
            if let Ok(center_group) = group_query.get(*center_group_entity) {
                if center_group.entities.len() == 1 {
                    if let Ok((transform, object_groups)) =
                        object_query.get(center_group.entities[0])
                    {
                        Some(transform.translation.xy())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let amount = self.easing.sample(progress) - self.easing.sample(previous_progress);

        let delta = Quat::from_rotation_z(
            -((360 * self.times360 + self.degrees) as f32 * amount).to_radians(),
        );

        if let Some(center) = center {
            global_group_delta
                .deltas
                .push(TransformDelta::RotateAround {
                    center,
                    rotation: delta,
                    lock_rotation: self.lock_rotation,
                })
        } else {
            global_group_delta.rotation *= delta;
        }
    }

    fn duration(&self) -> f32 {
        self.duration
    }

    fn exclusive(&self) -> bool {
        false
    }
}
