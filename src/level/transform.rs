use bevy::ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::system::SystemChangeTick;
use bevy::hierarchy::{Children, Parent};
use bevy::prelude::{GlobalTransform, Mut, Query, Ref, Res, Transform, With, Without};
use bevy::tasks::ComputeTaskPool;
use std::sync::atomic::Ordering;

use crate::level::section::GlobalSections;

pub(crate) fn update_transform(
    global_sections: Res<GlobalSections>,
    object_query: Query<(Ref<Transform>, &mut GlobalTransform, Option<&Children>), Without<Parent>>,
    children_query: Query<(&Transform, &mut GlobalTransform, Option<&Children>), With<Parent>>,
    system_change_tick: SystemChangeTick,
) {
    let visible_sections = unsafe { &*global_sections.visible.1.get() };

    let sections_to_update = &visible_sections[..global_sections.visible.0.load(Ordering::Relaxed)];

    let compute_task_pool = ComputeTaskPool::get();

    let thread_chunk_size = (sections_to_update.len() / compute_task_pool.thread_num()).max(1);

    let object_query = &object_query;
    let children_query = &children_query;
    let system_change_tick = &system_change_tick;

    compute_task_pool.scope(|scope| {
        for thread_chunk in sections_to_update.chunks(thread_chunk_size) {
            scope.spawn(async move {
                for section in thread_chunk {
                    let section = unsafe { section.assume_init() };
                    let mut iter = unsafe { object_query.iter_many_unsafe(section) };
                    while let Some((transform, mut global_transform, children)) = iter.fetch_next()
                    {
                        if !transform.last_changed().is_newer_than(
                            global_transform.last_changed(),
                            system_change_tick.this_run(),
                        ) {
                            continue;
                        }

                        *global_transform = GlobalTransform::from(*transform);

                        let Some(children) = children else {
                            continue;
                        };

                        unsafe {
                            propagate_transform_recursive(
                                children,
                                &children_query,
                                &global_transform,
                            );
                        }
                    }
                }
            });
        }
    });
}

unsafe fn propagate_transform_recursive<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery>(
    children: &Children,
    children_query: &'w Query<'w, 's, Q, F>,
    parent_transform: &GlobalTransform,
) where
    Q: WorldQuery<
        Item<'w> = (
            &'w Transform,
            Mut<'w, GlobalTransform>,
            Option<&'w Children>,
        ),
    >,
{
    for child in children {
        let Ok((transform, mut global_transform, children)) = children_query.get_unchecked(*child)
        else {
            continue;
        };

        *global_transform = parent_transform.mul_transform(*transform);

        let Some(children) = children else {
            continue;
        };

        propagate_transform_recursive(children, children_query, &global_transform)
    }
}
