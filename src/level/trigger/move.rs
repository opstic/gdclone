use crate::level::easing::Easing;
use crate::level::object::Object;
use crate::level::trigger::{Trigger, TriggerDuration, TriggerFunction};
use crate::level::Groups;
use crate::states::play::Player;
use bevy::ecs::system::SystemState;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{Query, Res, Transform, With, Without, World};
use bevy::time::Time;

#[derive(Clone, Default)]
pub(crate) struct MoveTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) easing: Easing,
    pub(crate) target_group: u32,
    pub(crate) x_offset: f32,
    pub(crate) y_offset: f32,
    pub(crate) lock_x: bool,
    pub(crate) lock_y: bool,
    pub(crate) player_previous_translation: Vec2,
}

impl TriggerFunction for MoveTrigger {
    fn execute(&mut self, world: &mut World) {
        let mut system_state: SystemState<(
            Res<Time>,
            Res<Groups>,
            Query<&mut Transform, (With<Object>, Without<Trigger>)>,
            Query<&mut Transform, (With<Player>, Without<Object>)>,
        )> = SystemState::new(world);
        let (time, groups, mut object_transform_query, player_transform_query) =
            system_state.get_mut(world);

        let mut player_translation = Vec2::default();
        if self.lock_x || self.lock_y {
            player_translation = player_transform_query
                .get_single()
                .unwrap()
                .translation
                .xy();
            if self.player_previous_translation == Vec2::ZERO {
                self.player_previous_translation = player_translation;
            }
        }

        let mut amount = self.easing.sample(self.duration.fraction_progress());
        self.duration.tick(time.delta());
        if self.duration.duration.is_zero() {
            amount = 1.;
        } else {
            amount = self.easing.sample(self.duration.fraction_progress()) - amount;
        }
        if let Some(group) = groups.0.get(&self.target_group) {
            for entity in &group.entities {
                if let Ok(mut transform) = object_transform_query.get_mut(*entity) {
                    transform.translation += Vec2::new(
                        (amount * self.x_offset as f64 * 4.) as f32,
                        (amount * self.y_offset as f64 * 4.) as f32,
                    )
                    .extend(0.);
                    if self.lock_x {
                        transform.translation.x +=
                            player_translation.x - self.player_previous_translation.x;
                    }
                    if self.lock_y {
                        transform.translation.y +=
                            player_translation.y - self.player_previous_translation.y;
                    }
                }
            }
        }
        self.player_previous_translation = player_translation;
    }

    fn get_target_group(&self) -> u32 {
        self.target_group
    }

    fn done_executing(&self) -> bool {
        self.duration.completed()
    }
}
