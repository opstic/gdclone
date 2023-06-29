use bevy::ecs::system::SystemState;
use bevy::math::{Quat, Vec3Swizzles};
use bevy::prelude::{Query, Res, ResMut, Transform, With, Without, World};
use bevy::time::Time;

use crate::level::{
    easing::Easing,
    object::Object,
    trigger::{Trigger, TriggerDuration, TriggerFunction},
    Groups, Sections,
};
use crate::utils::section_from_pos;

#[derive(Clone, Debug, Default)]
pub(crate) struct RotateTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
    pub(crate) center_group: u64,
    pub(crate) degrees: i32,
    pub(crate) times360: i32,
}

impl TriggerFunction for RotateTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<(
            Res<Time>,
            Res<Groups>,
            ResMut<Sections>,
            Query<&mut Transform, (With<Object>, Without<Trigger>)>,
        )> = SystemState::new(world);
        let (time, groups, mut sections, mut object_transform_query) = system_state.get_mut(world);
        let mut amount = self.easing.sample(self.duration.fraction_progress());
        self.duration.tick(time.delta());
        if self.duration.duration.is_zero() {
            amount = 1.;
        } else {
            amount = self.easing.sample(self.duration.fraction_progress()) - amount;
        }
        let center_translation =
            if let Some((center_group, _, _)) = groups.0.get(&self.center_group) {
                if center_group.entities.len() == 1 {
                    object_transform_query
                        .get(center_group.entities[0])
                        .map(|transform| transform.translation.xy())
                        .ok()
                } else {
                    None
                }
            } else {
                None
            };
        let rotation_amount = Quat::from_rotation_z(
            -((360 * self.times360 + self.degrees) as f32 * amount).to_radians(),
        );
        if let Some((group, _, _)) = groups.0.get(&self.target_group) {
            for entity in &group.entities {
                if let Ok(mut transform) = object_transform_query.get_mut(*entity) {
                    let initial_section = section_from_pos(transform.translation.xy());
                    if let Some(center_translation) = center_translation {
                        transform.rotate_around(center_translation.extend(0.), rotation_amount);
                    } else {
                        transform.rotate(rotation_amount);
                    }
                    let after_section = section_from_pos(transform.translation.xy());
                    if initial_section != after_section {
                        sections.get_section_mut(&initial_section).remove(entity);
                        sections.get_section_mut(&after_section).insert(*entity);
                    }
                }
            }
        }
    }

    fn get_target_group(&self) -> u64 {
        self.target_group
    }

    fn done_executing(&self) -> bool {
        self.duration.completed() || self.duration.duration.is_zero()
    }

    fn exclusive(&self) -> bool {
        !self.duration.duration.is_zero()
    }
}
