use std::f32::consts::PI;

use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::hierarchy::Parent;
use bevy::math::Vec2;
use bevy::prelude::{Children, Component, EntityWorldMut, Mut, Query, Res, With, Without};
use bevy::tasks::ComputeTaskPool;
use bevy::time::Time;
use bevy::utils::HashMap;

use crate::level::color::{ObjectColor, ObjectColorCalculated};
use crate::level::easing::Easing;
use crate::level::section::GlobalSections;
use crate::level::transform::Transform2d;
use crate::utils::str_to_bool;

#[derive(Component)]
pub(crate) enum Animation {
    Rotation(f32),
    ScaleAndFade(bool, f32, Vec2),
}

pub(crate) fn insert_animation_data(
    entity_world_mut: &mut EntityWorldMut,
    object_id: u64,
    object_data: &HashMap<&str, &str>,
) -> Result<(), anyhow::Error> {
    match object_id {
        740 | 1705 | 741 | 742 | 1706 | 675 | 676 | 678 | 1707 | 679 | 1708 | 680 | 1709 | 1619
        | 1710 | 1620 | 1734 | 183 | 1735 | 184 | 1736 | 185 | 186 | 85 | 187 | 188 | 86 | 97
        | 137 | 138 | 139 | 154 | 155 | 156 | 180 | 181 | 182 | 222 | 224 | 223 | 375 | 376
        | 377 | 378 | 1521 | 1522 | 1524 | 1525 | 1523 | 1526 | 1527 | 1528 | 394 | 395 | 1000
        | 396 | 997 | 1019 | 998 | 999 | 1020 | 1021 | 1055 | 1056 | 1057 | 1058 | 1059 | 1060
        | 1061 | 1752 | 1832 | 1831 | 1833 | 1022 | 1330 => {
            let mut amount =
                PI * (fastrand::f32() * 0.5 + 0.5) * if fastrand::bool() { 1. } else { -1. };
            if let Some(disable) = object_data.get("98") {
                if str_to_bool(disable) {
                    return Ok(());
                }
            }
            if let Some(custom) = object_data.get("97") {
                amount = custom.parse::<f32>()?.to_radians();
            }

            entity_world_mut.insert(Animation::Rotation(amount));
            Ok(())
        }
        1839 | 1840 | 1841 | 1842 => {
            let mut animation_speed = 1.;
            let mut randomize_start = false;
            if let Some(randomize) = object_data.get("106") {
                randomize_start = str_to_bool(randomize);
            }
            if let Some(speed) = object_data.get("107") {
                animation_speed = speed.parse()?;
            };

            animation_speed *= animation_speed;

            entity_world_mut.insert(Animation::ScaleAndFade(
                randomize_start,
                animation_speed,
                Vec2::ZERO,
            ));
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn update_animation(
    global_sections: Res<GlobalSections>,
    animates: Query<
        (
            &ObjectColorCalculated,
            &mut Transform2d,
            &mut ObjectColor,
            &mut Animation,
            Option<&Children>,
        ),
        Without<Parent>,
    >,
    animates_children: Query<(&mut ObjectColor, Option<&Children>), With<Parent>>,
    time: Res<Time>,
) {
    let sections_to_update = &global_sections.sections[global_sections.visible.clone()];

    let compute_task_pool = ComputeTaskPool::get();

    let thread_chunk_size = (sections_to_update.len() / compute_task_pool.thread_num()).max(1);

    let time = &time;
    let animates = &animates;
    let animates_children = &animates_children;

    compute_task_pool.scope(|scope| {
        for thread_chunk in sections_to_update.chunks(thread_chunk_size) {
            scope.spawn(async move {
                for section in thread_chunk {
                    let mut iter = unsafe { animates.iter_many_unsafe(section) };
                    while let Some((
                        calculated,
                        mut transform,
                        mut object_color,
                        mut animation,
                        children,
                    )) = iter.fetch_next()
                    {
                        if !calculated.enabled {
                            continue;
                        }
                        match &mut *animation {
                            Animation::Rotation(amount) => {
                                transform.angle += *amount * time.delta_seconds();
                            }
                            Animation::ScaleAndFade(randomize_start, speed, original_scale) => {
                                if *original_scale == Vec2::ZERO {
                                    *original_scale = transform.scale;
                                    if *randomize_start {
                                        let start = fastrand::f32();
                                        transform.scale *= start;
                                    }
                                }

                                transform.scale += *original_scale * *speed * time.delta_seconds();

                                transform.scale %= *original_scale;

                                object_color.object_opacity = Easing::ExponentialOut
                                    .sample(1. - (transform.scale.x / original_scale.x));

                                let Some(children) = children else {
                                    continue;
                                };

                                unsafe {
                                    propagate_opacity_recursive(
                                        children,
                                        animates_children,
                                        object_color.object_opacity,
                                    );
                                }
                            }
                        }
                    }
                }
            })
        }
    });
}

unsafe fn propagate_opacity_recursive<'w, 's, D: QueryData, F: QueryFilter>(
    children: &Children,
    children_query: &'w Query<'w, 's, D, F>,
    opacity: f32,
) where
    D: QueryData<Item<'w> = (Mut<'w, ObjectColor>, Option<&'w Children>)>,
{
    for child in children {
        let Ok((mut object_color, children)) = children_query.get_unchecked(*child) else {
            continue;
        };

        object_color.object_opacity = opacity;

        let Some(children) = children else {
            continue;
        };

        propagate_opacity_recursive(children, children_query, opacity);
    }
}
