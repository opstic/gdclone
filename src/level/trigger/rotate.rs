use crate::level::easing::Easing;
use crate::level::object::Object;
use crate::level::trigger::{Trigger, TriggerDuration, TriggerFunction};
use crate::level::Groups;
use bevy::ecs::system::SystemState;
use bevy::math::{Quat, Vec3Swizzles};
use bevy::prelude::{Query, Res, Transform, With, Without, World};
use bevy::time::Time;

#[derive(Clone, Default)]
pub(crate) struct RotateTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) easing: Easing,
    pub(crate) target_group: u32,
    pub(crate) center_group: u32,
    pub(crate) degrees: i32,
    pub(crate) times360: i32,
}

impl TriggerFunction for RotateTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<(
            Res<Time>,
            Res<Groups>,
            Query<&mut Transform, (With<Object>, Without<Trigger>)>,
        )> = SystemState::new(world);
        let (time, groups, mut object_transform_query) = system_state.get_mut(world);
        let mut amount = self.easing.sample(self.duration.fraction_progress());
        self.duration.tick(time.delta());
        if self.duration.duration.is_zero() {
            amount = 1.;
        } else {
            amount = self.easing.sample(self.duration.fraction_progress()) - amount;
        }
        let center_translation = if let Some(center_group) = groups.0.get(&self.center_group) {
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
        if let Some(center_translation) = center_translation {
            if let Some(group) = groups.0.get(&self.target_group) {
                for entity in &group.entities {
                    if let Ok(mut transform) = object_transform_query.get_mut(*entity) {
                        transform.rotate_around(
                            center_translation.extend(0.),
                            Quat::from_rotation_z(
                                -((360 * self.times360 + self.degrees) as f64 * amount).to_radians()
                                    as f32,
                            ),
                        );
                    }
                }
            }
        } else if let Some(group) = groups.0.get(&self.target_group) {
            for entity in &group.entities {
                if let Ok(mut transform) = object_transform_query.get_mut(*entity) {
                    transform.rotate(Quat::from_rotation_z(
                        -((360 * self.times360 + self.degrees) as f64 * amount).to_radians() as f32,
                    ));
                }
            }
        }
    }

    fn get_target_group(&self) -> u32 {
        self.target_group
    }

    fn done_executing(&self) -> bool {
        self.duration.completed()
    }
}
