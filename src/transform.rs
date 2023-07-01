use crate::level::AlreadyVisible;
use crate::loader::cocos2d_atlas::Cocos2dAtlasSprite;
use bevy::app::{App, CoreSchedule, CoreSet, Plugin, StartupSet};
use bevy::ecs::schedule::SystemSet;
use bevy::hierarchy::{Children, Parent, ValidParentCheckPlugin};
use bevy::prelude::{
    DetectChanges, Entity, GlobalTransform, IntoSystemConfig, IntoSystemSetConfig, Query, Ref,
    ResMut, Transform, With, Without,
};
use bevy::render::view::VisibleEntities;
use bevy::transform::{systems::sync_simple_transforms, TransformSystem};

use crate::utils::PassHashSet;

/// The base plugin for handling [`Transform`] components
#[derive(Default)]
pub(crate) struct CustomTransformPlugin;

impl Plugin for CustomTransformPlugin {
    fn build(&self, app: &mut App) {
        // A set for `propagate_transforms` to mark it as ambiguous with `sync_simple_transforms`.
        // Used instead of the `SystemTypeSet` as that would not allow multiple instances of the system.
        #[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
        struct PropagateTransformsSet;

        app.register_type::<Transform>()
            .register_type::<GlobalTransform>()
            .add_plugin(ValidParentCheckPlugin::<GlobalTransform>::default())
            // add transform systems to startup so the first update is "correct"
            .configure_set(TransformSystem::TransformPropagate.in_base_set(CoreSet::PostUpdate))
            .configure_set(PropagateTransformsSet.in_set(TransformSystem::TransformPropagate))
            .edit_schedule(CoreSchedule::Startup, |schedule| {
                schedule.configure_set(
                    TransformSystem::TransformPropagate.in_base_set(StartupSet::PostStartup),
                );
            })
            // FIXME: https://github.com/bevyengine/bevy/issues/4381
            // These systems cannot access the same entities,
            // due to subtle query filtering that is not yet correctly computed in the ambiguity detector
            .add_startup_system(
                sync_simple_transforms
                    .in_set(TransformSystem::TransformPropagate)
                    .ambiguous_with(PropagateTransformsSet),
            )
            .add_startup_system(propagate_transforms.in_set(PropagateTransformsSet))
            .add_startup_system(propagate_atlas_transforms.in_set(PropagateTransformsSet))
            .add_system(
                sync_simple_transforms
                    .in_set(TransformSystem::TransformPropagate)
                    .ambiguous_with(PropagateTransformsSet),
            )
            .add_system(propagate_transforms.in_set(PropagateTransformsSet))
            .add_system(propagate_atlas_transforms.in_set(PropagateTransformsSet));
    }
}

pub(crate) fn propagate_atlas_transforms(
    mut root_query: Query<
        (Entity, &Children, Ref<Transform>, &mut GlobalTransform),
        (With<Cocos2dAtlasSprite>, Without<Parent>),
    >,
    transform_query: Query<
        (Ref<Transform>, &mut GlobalTransform, Option<&Children>),
        (With<Cocos2dAtlasSprite>, With<Parent>),
    >,
    parent_query: Query<(Entity, Ref<Parent>), With<Cocos2dAtlasSprite>>,
    visible_entities_query: Query<&VisibleEntities>,
    mut already_visible: ResMut<AlreadyVisible>,
) {
    let mut all_visible = PassHashSet::default();
    for visible_entities in &visible_entities_query {
        all_visible.extend(
            visible_entities
                .entities
                .iter()
                .map(|entity| entity.index() as u64),
        );
    }

    let mut newly_visible = PassHashSet::default();

    for entity_index in &all_visible {
        if already_visible.0.contains(entity_index) {
            continue;
        }
        newly_visible.insert(*entity_index);
        already_visible.0.insert(*entity_index);
    }

    root_query.par_iter_mut().for_each_mut(
        |(entity, children, transform, mut global_transform)| {
            let changed = transform.is_changed() || newly_visible.contains(&(entity.index() as u64));
            if changed {
                *global_transform = GlobalTransform::from(*transform);
            }

            if !all_visible.contains(&(entity.index() as u64)) {
                return;
            }

            for (child, actual_parent) in parent_query.iter_many(children) {
                assert_eq!(
                    actual_parent.get(), entity,
                    "Malformed hierarchy. This probably means that your hierarchy has been improperly maintained, or contains a cycle"
                );
                // SAFETY:
                // - `child` must have consistent parentage, or the above assertion would panic.
                // Since `child` is parented to a root entity, the entire hierarchy leading to it is consistent.
                // - We may operate as if all descendants are consistent, since `propagate_recursive` will panic before
                //   continuing to propagate if it encounters an entity with inconsistent parentage.
                // - Since each root entity is unique and the hierarchy is consistent and forest-like,
                //   other root entities' `propagate_recursive` calls will not conflict with this one.
                // - Since this is the only place where `transform_query` gets used, there will be no conflicting fetches elsewhere.
                unsafe {
                    propagate_atlas_recursive(
                        &global_transform,
                        &transform_query,
                        &parent_query,
                        child,
                        changed || actual_parent.is_changed(),
                    );
                }
            }
        },
    );

    already_visible
        .0
        .retain(|index| all_visible.contains(index));
}

/// Recursively propagates the transforms for `entity` and all of its descendants.
///
/// # Panics
///
/// If `entity`'s descendants have a malformed hierarchy, this function will panic occur before propagating
/// the transforms of any malformed entities and their descendants.
///
/// # Safety
///
/// - While this function is running, `transform_query` must not have any fetches for `entity`,
/// nor any of its descendants.
/// - The caller must ensure that the hierarchy leading to `entity`
/// is well-formed and must remain as a tree or a forest. Each entity must have at most one parent.
unsafe fn propagate_atlas_recursive(
    parent: &GlobalTransform,
    transform_query: &Query<
        (Ref<Transform>, &mut GlobalTransform, Option<&Children>),
        (With<Cocos2dAtlasSprite>, With<Parent>),
    >,
    parent_query: &Query<(Entity, Ref<Parent>), With<Cocos2dAtlasSprite>>,
    entity: Entity,
    mut changed: bool,
) {
    let (global_matrix, children) = {
        let Ok((transform, mut global_transform, children)) =
            // SAFETY: This call cannot create aliased mutable references.
            //   - The top level iteration parallelizes on the roots of the hierarchy.
            //   - The caller ensures that each child has one and only one unique parent throughout the entire
            //     hierarchy.
            //
            // For example, consider the following malformed hierarchy:
            //
            //     A
            //   /   \
            //  B     C
            //   \   /
            //     D
            //
            // D has two parents, B and C. If the propagation passes through C, but the Parent component on D points to B,
            // the above check will panic as the origin parent does match the recorded parent.
            //
            // Also consider the following case, where A and B are roots:
            //
            //  A       B
            //   \     /
            //    C   D
            //     \ /
            //      E
            //
            // Even if these A and B start two separate tasks running in parallel, one of them will panic before attempting
            // to mutably access E.
            (unsafe { transform_query.get_unchecked(entity) }) else {
            return;
        };

        changed |= transform.is_changed();
        if changed {
            *global_transform = parent.mul_transform(*transform);
        }
        (*global_transform, children)
    };

    let Some(children) = children else { return; };
    for (child, actual_parent) in parent_query.iter_many(children) {
        assert_eq!(
            actual_parent.get(), entity,
            "Malformed hierarchy. This probably means that your hierarchy has been improperly maintained, or contains a cycle"
        );
        // SAFETY: The caller guarantees that `transform_query` will not be fetched
        // for any descendants of `entity`, so it is safe to call `propagate_recursive` for each child.
        //
        // The above assertion ensures that each child has one and only one unique parent throughout the
        // entire hierarchy.
        unsafe {
            propagate_atlas_recursive(
                &global_matrix,
                transform_query,
                parent_query,
                child,
                changed || actual_parent.is_changed(),
            );
        }
    }
}

/// Update [`GlobalTransform`] component of entities based on entity hierarchy and
/// [`Transform`] component.
///
/// Third party plugins should ensure that this is used in concert with [`sync_simple_transforms`].
pub(crate) fn propagate_transforms(
    mut root_query: Query<
        (Entity, &Children, Ref<Transform>, &mut GlobalTransform),
        (Without<Parent>, Without<Cocos2dAtlasSprite>),
    >,
    transform_query: Query<
        (Ref<Transform>, &mut GlobalTransform, Option<&Children>),
        (With<Parent>, Without<Cocos2dAtlasSprite>),
    >,
    parent_query: Query<(Entity, Ref<Parent>), Without<Cocos2dAtlasSprite>>,
) {
    root_query.par_iter_mut().for_each_mut(
        |(entity, children, transform, mut global_transform)| {
            let changed = transform.is_changed();
            if changed {
                *global_transform = GlobalTransform::from(*transform);
            }

            for (child, actual_parent) in parent_query.iter_many(children) {
                assert_eq!(
                    actual_parent.get(), entity,
                    "Malformed hierarchy. This probably means that your hierarchy has been improperly maintained, or contains a cycle"
                );
                // SAFETY:
                // - `child` must have consistent parentage, or the above assertion would panic.
                // Since `child` is parented to a root entity, the entire hierarchy leading to it is consistent.
                // - We may operate as if all descendants are consistent, since `propagate_recursive` will panic before 
                //   continuing to propagate if it encounters an entity with inconsistent parentage.
                // - Since each root entity is unique and the hierarchy is consistent and forest-like,
                //   other root entities' `propagate_recursive` calls will not conflict with this one.
                // - Since this is the only place where `transform_query` gets used, there will be no conflicting fetches elsewhere.
                unsafe {
                    propagate_recursive(
                        &global_transform,
                        &transform_query,
                        &parent_query,
                        child,
                        changed || actual_parent.is_changed(),
                    );
                }
            }
        },
    );
}

/// Recursively propagates the transforms for `entity` and all of its descendants.
///
/// # Panics
///
/// If `entity`'s descendants have a malformed hierarchy, this function will panic occur before propagating
/// the transforms of any malformed entities and their descendants.
///
/// # Safety
///
/// - While this function is running, `transform_query` must not have any fetches for `entity`,
/// nor any of its descendants.
/// - The caller must ensure that the hierarchy leading to `entity`
/// is well-formed and must remain as a tree or a forest. Each entity must have at most one parent.
unsafe fn propagate_recursive(
    parent: &GlobalTransform,
    transform_query: &Query<
        (Ref<Transform>, &mut GlobalTransform, Option<&Children>),
        (With<Parent>, Without<Cocos2dAtlasSprite>),
    >,
    parent_query: &Query<(Entity, Ref<Parent>), Without<Cocos2dAtlasSprite>>,
    entity: Entity,
    mut changed: bool,
) {
    let (global_matrix, children) = {
        let Ok((transform, mut global_transform, children)) =
            // SAFETY: This call cannot create aliased mutable references.
            //   - The top level iteration parallelizes on the roots of the hierarchy.
            //   - The caller ensures that each child has one and only one unique parent throughout the entire
            //     hierarchy.
            //
            // For example, consider the following malformed hierarchy:
            //
            //     A
            //   /   \
            //  B     C
            //   \   /
            //     D
            //
            // D has two parents, B and C. If the propagation passes through C, but the Parent component on D points to B,
            // the above check will panic as the origin parent does match the recorded parent.
            //
            // Also consider the following case, where A and B are roots:
            //
            //  A       B
            //   \     /
            //    C   D
            //     \ /
            //      E
            //
            // Even if these A and B start two separate tasks running in parallel, one of them will panic before attempting
            // to mutably access E.
            (unsafe { transform_query.get_unchecked(entity) }) else {
            return;
        };

        changed |= transform.is_changed();
        if changed {
            *global_transform = parent.mul_transform(*transform);
        }
        (*global_transform, children)
    };

    let Some(children) = children else { return };
    for (child, actual_parent) in parent_query.iter_many(children) {
        assert_eq!(
            actual_parent.get(), entity,
            "Malformed hierarchy. This probably means that your hierarchy has been improperly maintained, or contains a cycle"
        );
        // SAFETY: The caller guarantees that `transform_query` will not be fetched
        // for any descendants of `entity`, so it is safe to call `propagate_recursive` for each child.
        //
        // The above assertion ensures that each child has one and only one unique parent throughout the
        // entire hierarchy.
        unsafe {
            propagate_recursive(
                &global_matrix,
                transform_query,
                parent_query,
                child,
                changed || actual_parent.is_changed(),
            );
        }
    }
}
