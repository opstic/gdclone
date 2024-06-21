use std::any::{Any, TypeId};
use std::f32::consts::FRAC_PI_2;

use bevy::input::ButtonInput;
use bevy::math::{DVec2, Vec2, Vec3Swizzles};
use bevy::prelude::{
    Component, Entity, Event, EventWriter, MouseButton, Mut, Query, Res, Resource, With, Without,
    World,
};
use bevy::time::Time;
use bevy::utils::HashMap;

use crate::level::collision::{ActiveCollider, GlobalHitbox, GlobalHitboxKind, Hitbox};
use crate::level::object::ObjectType;
use crate::level::player_function::mode::cube::CubeMode;
use crate::level::player_function::{GameplayObject, PlayerFunction};
use crate::level::transform::{GlobalTransform2d, Transform2d};
use crate::level::trigger::{Activated, GlobalTriggers, SpeedChange};

#[derive(Component)]
pub(crate) struct Player {
    pub(crate) last_translation: Vec2,
    pub(crate) velocity: DVec2,
    pub(crate) vertical_is_x: bool,
    pub(crate) flipped: bool,
    pub(crate) reverse: bool,
    pub(crate) mini: bool,
    pub(crate) speed: f64,
    pub(crate) inner_hitbox: Hitbox,
    pub(crate) on_ground: bool,
    pub(crate) game_mode: Box<dyn PlayerFunction>,
    pub(crate) pad_activated_frame: bool,
    pub(crate) orb_activated_frame: bool,
    pub(crate) buffered_input: bool,
    pub(crate) dash: Option<f32>,
    pub(crate) do_ceiling_collision: bool,
    pub(crate) hitbox_scale: Option<f32>,
    pub(crate) snap_distance: (f32, f32),
    pub(crate) disable_snap: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            last_translation: Vec2::ZERO,
            // velocity: Vec2::new(0., 0.),
            velocity: DVec2::new(5.77 * 60., 0.),
            vertical_is_x: false,
            flipped: false,
            reverse: false,
            mini: false,
            speed: 0.9,
            inner_hitbox: Hitbox::Box {
                no_rotation: true,
                offset: None,
                half_extents: Vec2::splat(3.75),
            },
            on_ground: false,
            game_mode: Box::new(CubeMode::default()),
            pad_activated_frame: false,
            orb_activated_frame: false,
            buffered_input: false,
            dash: None,
            do_ceiling_collision: false,
            hitbox_scale: None,
            snap_distance: (5., 9.),
            disable_snap: false,
        }
    }
}

impl Player {
    pub(crate) fn falling(&self) -> bool {
        self.velocity.y < 0.
    }

    pub(crate) fn rising(&self) -> bool {
        self.velocity.y > 0.
    }

    pub(crate) fn get_scaled_hitboxes(&self, hitbox: &Hitbox) -> (Hitbox, Hitbox) {
        let scale = self.hitbox_scale.unwrap_or(1.);
        (*hitbox * scale, self.inner_hitbox * scale)
    }

    pub(crate) fn calculate_player_global_hitbox(
        &self,
        transform: &Transform2d,
        global_transform: &GlobalTransform2d,
        hitbox: &Hitbox,
    ) -> (GlobalHitbox, GlobalHitbox, GlobalHitbox) {
        let (scaled_hitbox, scaled_inner_hitbox) = self.get_scaled_hitboxes(hitbox);
        (
            GlobalHitbox::from((hitbox, transform, global_transform)),
            GlobalHitbox::from((&scaled_hitbox, transform, global_transform)),
            GlobalHitbox::from((&scaled_inner_hitbox, transform, global_transform)),
        )
    }
}

pub(crate) const JUMP_HEIGHT: f64 = 11.180032;

#[derive(Event)]
pub(crate) struct KillPlayer(Entity, Option<Entity>);

#[derive(Default, Resource)]
pub(crate) struct PlayerFunctionSystemStateCache {
    system_states: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

pub(crate) fn update_player_game_mode(world: &mut World) {
    world.resource_scope(
        |world, mut system_state_cache: Mut<PlayerFunctionSystemStateCache>| {
            let world_cell = world.as_unsafe_world_cell();
            let mut query =
                unsafe { world_cell.world_mut() }.query::<(Entity, &mut Player, &ActiveCollider)>();
            let mut gameplay_objects =
                unsafe { world_cell.world_mut() }
                    .query_filtered::<(Entity, &mut GameplayObject), Without<Activated>>();

            let world_mut = unsafe { world_cell.world_mut() };

            let (just_pressed, pressed, just_released) =
                world_mut.resource_scope(|_, mouse_input: Mut<ButtonInput<MouseButton>>| {
                    (
                        mouse_input.just_pressed(MouseButton::Left),
                        mouse_input.pressed(MouseButton::Left),
                        mouse_input.just_released(MouseButton::Left),
                    )
                });

            for (entity, mut player, collider) in query.iter_mut(unsafe { world_cell.world_mut() })
            {
                if just_pressed {
                    player.buffered_input = true;
                } else if just_released {
                    player.buffered_input = false;
                }

                if !pressed {
                    player.dash = None;
                }

                player.pad_activated_frame = false;
                player.orb_activated_frame = false;

                let mut iter = gameplay_objects.iter_many_mut(
                    unsafe { world_cell.world_mut() },
                    collider
                        .collided
                        .iter()
                        .filter(|(_, _, _, object_type)| *object_type == ObjectType::Other)
                        .map(|(entity, _, _, _)| *entity),
                );

                while let Some((object_entity, mut gameplay_object)) = iter.fetch_next() {
                    let system_state = if let Some(system_state) = system_state_cache
                        .system_states
                        .get_mut(&gameplay_object.0.concrete_type_id())
                    {
                        system_state
                    } else {
                        system_state_cache.system_states.insert(
                            gameplay_object.0.concrete_type_id(),
                            gameplay_object.0.create_system_state(world_mut),
                        );

                        system_state_cache
                            .system_states
                            .get_mut(&gameplay_object.0.concrete_type_id())
                            .unwrap()
                    };

                    gameplay_object
                        .0
                        .update(world_mut, object_entity, entity, system_state);
                }

                let system_state = if let Some(system_state) = system_state_cache
                    .system_states
                    .get_mut(&player.game_mode.concrete_type_id())
                {
                    system_state
                } else {
                    system_state_cache.system_states.insert(
                        player.game_mode.concrete_type_id(),
                        player.game_mode.create_system_state(world_mut),
                    );

                    system_state_cache
                        .system_states
                        .get_mut(&player.game_mode.concrete_type_id())
                        .unwrap()
                };

                player
                    .game_mode
                    .update(world_mut, entity, entity, system_state);
            }
        },
    )
}

#[derive(Component)]
pub(crate) struct Ground;

pub(crate) fn update_player_pos(
    mut players: Query<(&mut Player, &mut Transform2d), Without<Ground>>,
    mut ground: Query<&mut Transform2d, With<Ground>>,
    speed_changes: Query<&SpeedChange>,
    time: Res<Time>,
    triggers: Res<GlobalTriggers>,
) {
    for (mut player, mut transform) in &mut players {
        let (_, speed_data) = triggers
            .speed_changes
            .speed_data_at_pos(transform.translation.x);
        let speed_change = speed_changes.get(speed_data.entity).unwrap();
        player.velocity.x = speed_change.forward_velocity as f64;
        player.speed = speed_change.speed as f64;

        player.last_translation = transform.translation.xy();

        transform.translation.x +=
            (player.velocity.x * time.delta_seconds_f64() * player.speed) as f32;

        if let Some(dash_direction) = player.dash {
            player.on_ground = false;
            player.velocity.y = 0.;
            transform.translation.y += (transform.translation.x - player.last_translation.x)
                / (-dash_direction - FRAC_PI_2).tan();
            transform.angle = dash_direction;
            continue;
        }

        if !player.flipped {
            transform.translation.y +=
                (player.velocity.y * 60. * 0.9 * time.delta_seconds_f64()) as f32;
        } else {
            transform.translation.y -=
                (player.velocity.y * 60. * 0.9 * time.delta_seconds_f64()) as f32;
        }
    }

    let mut ground_transform = ground.single_mut();
    let (_, player_transform) = players.single();

    ground_transform.translation.x = player_transform.translation.x;
}

pub(crate) fn process_player_collisions(
    mut ev_kill: EventWriter<KillPlayer>,
    mut players: Query<(
        Entity,
        &mut Player,
        &mut Transform2d,
        &mut GlobalTransform2d,
        &Hitbox,
        &ActiveCollider,
    )>,
) {
    for (entity, mut player, mut transform, mut global_transform, hitbox, active_collider) in
        &mut players
    {
        if active_collider.collided.is_empty() {
            continue;
        }

        let (mut global_hitbox, mut scaled_global_hitbox, mut global_inner_hitbox) =
            player.calculate_player_global_hitbox(&transform, &global_transform, hitbox);

        let player_hitbox_height = -global_hitbox.aabb.w - global_hitbox.aabb.y;

        for (collided_entity, collided_hitbox, collided_vertex, object_type) in
            &active_collider.collided
        {
            if !global_hitbox.intersect(collided_hitbox).0 {
                continue;
            }

            match *object_type {
                ObjectType::Solid => (),
                ObjectType::Hazard if scaled_global_hitbox.intersect(collided_hitbox).0 => {
                    ev_kill.send(KillPlayer(entity, Some(*collided_entity)));
                    // info!("killing");
                    continue;
                }
                _ => continue,
            }

            if !player.disable_snap {
                let do_ceiling_snap = if !player.flipped {
                    transform.translation.y <= collided_hitbox.aabb.y - player.snap_distance.1
                } else {
                    transform.translation.y >= -collided_hitbox.aabb.w + player.snap_distance.1
                };
                if player.do_ceiling_collision
                    && player.rising()
                    && do_ceiling_snap
                    && !collided_hitbox
                        .specific
                        .map(|specific| matches!(specific, GlobalHitboxKind::Triangle { .. }))
                        .unwrap_or_default()
                {
                    if !player.flipped {
                        transform.translation.y =
                            collided_hitbox.aabb.y - player_hitbox_height / 2.;
                    } else {
                        transform.translation.y =
                            -collided_hitbox.aabb.w + player_hitbox_height / 2.;
                    }
                    player.velocity.y = 0.;

                    *global_transform = GlobalTransform2d::from(*transform);
                    (global_hitbox, scaled_global_hitbox, global_inner_hitbox) = player
                        .calculate_player_global_hitbox(&transform, &global_transform, hitbox);
                }

                let do_snap = if !player.flipped {
                    transform.translation.y >= -collided_hitbox.aabb.w + player.snap_distance.0
                } else {
                    transform.translation.y <= collided_hitbox.aabb.y - player.snap_distance.0
                };
                if player.falling() && do_snap {
                    if !player.flipped {
                        transform.translation.y =
                            -collided_hitbox.aabb.w + player_hitbox_height / 2.;
                    } else {
                        transform.translation.y =
                            collided_hitbox.aabb.y - player_hitbox_height / 2.;
                    }
                    player.velocity.y = 0.;
                    player.on_ground = true;

                    *global_transform = GlobalTransform2d::from(*transform);
                    (global_hitbox, scaled_global_hitbox, global_inner_hitbox) = player
                        .calculate_player_global_hitbox(&transform, &global_transform, hitbox);
                }
            }

            if global_inner_hitbox.intersect(collided_hitbox).0 {
                ev_kill.send(KillPlayer(entity, Some(*collided_entity)));
                // info!("killing");
                // continue;
            }
        }
    }
}
