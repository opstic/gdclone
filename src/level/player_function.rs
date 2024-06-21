use std::any::{Any, TypeId};

use bevy::prelude::{Component, Entity, EntityWorldMut, World};

use crate::level::collision::Hitbox;
use crate::level::player::JUMP_HEIGHT;
use crate::level::player_function::dash_orb::DashOrb;
use crate::level::player_function::gravity_pad::GravityPad;
use crate::level::player_function::mode::ball::BallMode;
use crate::level::player_function::mode::cube::CubeMode;
use crate::level::player_function::mode::robot::RobotMode;
use crate::level::player_function::mode::ship::ShipMode;
use crate::level::player_function::mode::ufo::UfoMode;
use crate::level::player_function::mode::wave::WaveMode;
use crate::level::player_function::orb::Orb;
use crate::level::player_function::pad::Pad;
use crate::level::player_function::portal::Portal;
use crate::level::player_function::teleport::Teleport;
use crate::utils::ObjectStorage;

mod dash_orb;
mod gravity_pad;
pub(crate) mod mode;
mod orb;
pub(crate) mod pad;
mod portal;
pub(crate) mod teleport;

pub(crate) trait PlayerFunction: Send + Sync + 'static {
    fn update(
        &mut self,
        world: &mut World,
        origin_entity: Entity,
        player_entity: Entity,
        system_state: &mut Box<dyn Any + Send + Sync>,
    );

    fn create_system_state(&self, world: &mut World) -> Box<dyn Any + Send + Sync>;

    fn concrete_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

#[derive(Component)]
pub(crate) struct GameplayObject(pub(crate) Box<dyn PlayerFunction>);

pub(crate) fn insert_gameplay_object_data(
    entity_world_mut: &mut EntityWorldMut,
    object_id: u64,
    object_data: &ObjectStorage,
) -> Result<(), anyhow::Error> {
    match object_id {
        35 | 140 | 1332 => {
            let function = Pad {
                func: match object_id {
                    35 => |player| {
                        player.velocity.y = 1. * 16.;
                        true
                    },
                    140 => |player| {
                        player.velocity.y = 0.65 * 16.;
                        true
                    },
                    1332 => |player| {
                        player.velocity.y = 1.25 * 16.;
                        true
                    },
                    _ => unreachable!(),
                },
            };
            entity_world_mut.insert(GameplayObject(Box::new(function)));
        }
        67 => {
            entity_world_mut.insert(GameplayObject(Box::new(GravityPad)));
        }
        36 | 84 | 141 | 1022 | 1330 | 1333 => {
            let function = Orb {
                compute_force: match object_id {
                    36 => |_| JUMP_HEIGHT,
                    84 => |player| {
                        player.flipped = !player.flipped;
                        -0.8 * JUMP_HEIGHT * 0.5
                    },
                    141 => |player| {
                        let factor = if player.game_mode.concrete_type_id()
                            == ShipMode.concrete_type_id()
                        {
                            0.37
                        } else if player.game_mode.concrete_type_id() == UfoMode.concrete_type_id()
                        {
                            0.42
                        } else if player.game_mode.concrete_type_id() == BallMode.concrete_type_id()
                        {
                            0.77
                        } else {
                            0.72
                        };
                        JUMP_HEIGHT * factor
                    },
                    1022 => |player| {
                        player.flipped = !player.flipped;
                        if player.game_mode.concrete_type_id() == ShipMode.concrete_type_id() {
                            JUMP_HEIGHT * 0.7
                        } else {
                            JUMP_HEIGHT
                        }
                    },
                    1330 => |player| {
                        if player.game_mode.concrete_type_id() == ShipMode.concrete_type_id() {
                            -14.
                        } else if player.game_mode.concrete_type_id() == UfoMode.concrete_type_id()
                        {
                            -11.2
                        } else {
                            -15.
                        }
                    },
                    1333 => |player| {
                        let factor = if player.game_mode.concrete_type_id()
                            == ShipMode.concrete_type_id()
                            && player.mini
                        {
                            1.4
                        } else if player.game_mode.concrete_type_id() == UfoMode.concrete_type_id()
                        {
                            if !player.mini {
                                1.02
                            } else {
                                1.36
                            }
                        } else if player.game_mode.concrete_type_id() == BallMode.concrete_type_id()
                        {
                            1.34
                        } else {
                            1.38
                        };
                        JUMP_HEIGHT * factor
                    },
                    _ => unreachable!(),
                },
            };
            entity_world_mut.insert(GameplayObject(Box::new(function)));

            if let Some(mut hitbox) = entity_world_mut.get_mut::<Hitbox>() {
                if let Hitbox::Box { no_rotation, .. } = &mut *hitbox {
                    *no_rotation = true
                }
            }
        }
        10 | 11 | 99 | 101 => {
            let function = Portal {
                func: match object_id {
                    10 => |player| {
                        if !player.flipped {
                            return false;
                        }
                        player.flipped = false;
                        player.velocity.y /= -2.;
                        true
                    },
                    11 => |player| {
                        if player.flipped {
                            return false;
                        }
                        player.flipped = true;
                        player.velocity.y /= -2.;
                        true
                    },
                    99 => |player| {
                        if !player.mini {
                            return false;
                        }
                        player.mini = false;
                        true
                    },
                    101 => |player| {
                        if player.mini {
                            return false;
                        }
                        player.mini = true;
                        true
                    },
                    _ => unreachable!(),
                },
            };
            entity_world_mut.insert(GameplayObject(Box::new(function)));
        }
        12 | 13 | 47 | 111 | 660 | 745 => {
            let function = Portal {
                func: match object_id {
                    12 => |player| {
                        player.game_mode = Box::new(CubeMode::default());
                        player.do_ceiling_collision = false;
                        player.hitbox_scale = None;
                        player.disable_snap = false;
                        true
                    },
                    13 => |player| {
                        player.game_mode = Box::new(ShipMode);
                        player.velocity.y /= 2.;
                        player.do_ceiling_collision = true;
                        player.hitbox_scale = None;
                        player.disable_snap = false;
                        true
                    },
                    47 => |player| {
                        player.game_mode = Box::new(BallMode);
                        player.do_ceiling_collision = true;
                        player.hitbox_scale = None;
                        player.disable_snap = false;
                        true
                    },
                    111 => |player| {
                        player.game_mode = Box::new(UfoMode);
                        player.do_ceiling_collision = true;
                        player.hitbox_scale = None;
                        player.disable_snap = false;
                        true
                    },
                    660 => |player| {
                        player.game_mode = Box::new(WaveMode);
                        player.do_ceiling_collision = true;
                        player.hitbox_scale = Some(0.4875);
                        player.disable_snap = true;
                        true
                    },
                    745 => |player| {
                        player.game_mode = Box::new(RobotMode::default());
                        player.do_ceiling_collision = false;
                        player.hitbox_scale = None;
                        true
                    },
                    _ => unreachable!(),
                },
            };
            entity_world_mut.insert(GameplayObject(Box::new(function)));
        }
        747 => {
            let mut function = Teleport::default();
            if let Some(distance) = object_data.get("54") {
                function.distance = distance.parse()?;
            }
            entity_world_mut.insert(GameplayObject(Box::new(function)));
        }
        1704 | 1751 => {
            let function = DashOrb {
                flip: match object_id {
                    1704 => false,
                    1751 => true,
                    _ => unreachable!(),
                },
            };
            entity_world_mut.insert(GameplayObject(Box::new(function)));
        }
        1755 | 1829 | 1859 => {
            let function = Portal {
                func: match object_id {
                    1755 => |player| {
                        player.disable_snap = false;
                        player.do_ceiling_collision = true;
                        true
                    },
                    1829 => |player| {
                        player.dash = None;
                        true
                    },
                    1859 => |player| {
                        player.do_ceiling_collision = true;
                        false
                    },
                    _ => unreachable!(),
                },
            };
            entity_world_mut.insert(GameplayObject(Box::new(function)));
        }
        _ => (),
    }
    Ok(())
}
