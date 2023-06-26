use bevy::ecs::system::SystemState;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{Query, Res, ResMut, Transform, With, Without, World};
use bevy::time::Time;

use crate::level::{Groups, Sections};
use crate::level::easing::Easing;
use crate::level::object::Object;
use crate::level::trigger::{Trigger, TriggerDuration, TriggerFunction};
use crate::states::play::Player;
use crate::utils::section_from_pos;

#[derive(Clone, Debug, Default)]
pub(crate) struct MoveTrigger {
    pub(crate) duration: TriggerDuration,
    pub(crate) easing: Easing,
    pub(crate) target_group: u64,
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
            ResMut<Sections>,
            Query<&mut Transform, (With<Object>, Without<Trigger>)>,
            Query<&mut Transform, (With<Player>, Without<Object>)>,
        )> = SystemState::new(world);
        let (time, groups, mut sections, mut object_transform_query, player_transform_query) =
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
        if let Some((group, _, _)) = groups.0.get(&self.target_group) {
            for entity in &group.entities {
                if let Ok(mut transform) = object_transform_query.get_mut(*entity) {
                    let initial_section = section_from_pos(transform.translation.xy());
                    let mut delta = Vec2::new(self.x_offset, self.y_offset) * amount;
                    if self.lock_x {
                        delta.x += player_translation.x - self.player_previous_translation.x;
                    }
                    if self.lock_y {
                        delta.y += player_translation.y - self.player_previous_translation.y;
                    }
                    transform.translation += delta.extend(0.);
                    let after_section = section_from_pos(transform.translation.xy());
                    if initial_section != after_section {
                        sections.get_section_mut(&initial_section).remove(entity);
                        sections.get_section_mut(&after_section).insert(*entity);
                    }
                }
            }
        }
        self.player_previous_translation = player_translation;
    }

    fn get_target_group(&self) -> u64 {
        self.target_group
    }

    fn done_executing(&self) -> bool {
        self.duration.completed()
    }

    fn exclusive(&self) -> bool {
        false
    }
}
