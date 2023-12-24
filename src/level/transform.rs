use bevy::ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy::hierarchy::{Children, Parent};
use bevy::prelude::{Changed, GlobalTransform, Mut, Query, Res, Transform, With, Without};

use crate::level::section::{GlobalSections, SectionIndex, VisibleGlobalSections};

pub(crate) fn update_transform(
    global_sections: Res<GlobalSections>,
    visible_global_sections: Res<VisibleGlobalSections>,
    mut object_query: Query<
        (&Transform, &mut GlobalTransform, Option<&Children>),
        (Changed<Transform>, Without<Parent>),
    >,
    children_query: Query<(&Transform, &mut GlobalTransform, Option<&Children>), With<Parent>>,
) {
    for x in visible_global_sections.x.clone() {
        for y in visible_global_sections.y.clone() {
            let section_index = SectionIndex::new(x, y);
            let Some(global_section) = global_sections.0.get(&section_index) else {
                continue;
            };

            let mut iter = object_query.iter_many_mut(global_section.value());
            while let Some((transform, mut global_transform, children)) = iter.fetch_next() {
                *global_transform = GlobalTransform::from(*transform);

                let Some(children) = children else {
                    continue;
                };

                unsafe {
                    propagate_transform_recursive(children, &children_query, &global_transform);
                }
            }
        }
    }
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
