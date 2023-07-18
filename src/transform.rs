use bevy::app::{App, Plugin, PostStartup, PostUpdate};
use bevy::ecs::schedule::SystemSet;
use bevy::hierarchy::{Children, Parent, ValidParentCheckPlugin};
use bevy::prelude::{
    DetectChanges, Entity, GlobalTransform, IntoSystemConfigs, IntoSystemSetConfig, Local, Query,
    Ref, RemovedComponents, Transform, With, Without,
};
use bevy::render::view::VisibleEntities;
use bevy::transform::{systems::sync_simple_transforms, TransformSystem};

use crate::loader::cocos2d_atlas::Cocos2dAtlasSprite;

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
            .add_plugins(ValidParentCheckPlugin::<GlobalTransform>::default())
            .configure_set(
                PostStartup,
                PropagateTransformsSet.in_set(TransformSystem::TransformPropagate),
            )
            // add transform systems to startup so the first update is "correct"
            .add_systems(
                PostStartup,
                (
                    sync_simple_transforms
                        .in_set(TransformSystem::TransformPropagate)
                        // FIXME: https://github.com/bevyengine/bevy/issues/4381
                        // These systems cannot access the same entities,
                        // due to subtle query filtering that is not yet correctly computed in the ambiguity detector
                        .ambiguous_with(PropagateTransformsSet),
                    propagate_transforms.in_set(PropagateTransformsSet),
                    propagate_atlas_transforms.in_set(PropagateTransformsSet),
                ),
            )
            .configure_set(
                PostUpdate,
                PropagateTransformsSet.in_set(TransformSystem::TransformPropagate),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_simple_transforms
                        .in_set(TransformSystem::TransformPropagate)
                        .ambiguous_with(PropagateTransformsSet),
                    propagate_transforms.in_set(PropagateTransformsSet),
                    propagate_atlas_transforms.in_set(PropagateTransformsSet),
                ),
            );
    }
}

pub(crate) fn propagate_atlas_transforms(
    mut root_query: Query<
        (Entity, &Children, Ref<Transform>, &mut GlobalTransform),
        (Without<Parent>, With<Cocos2dAtlasSprite>),
    >,
    mut orphaned: RemovedComponents<Parent>,
    transform_query: Query<
        (Ref<Transform>, &mut GlobalTransform, Option<&Children>),
        (With<Parent>, With<Cocos2dAtlasSprite>),
    >,
    parent_query: Query<(Entity, Ref<Parent>), With<Cocos2dAtlasSprite>>,
    mut orphaned_entities: Local<Vec<Entity>>,
    visible_entities_query: Query<&VisibleEntities>,
    mut all_visible: Local<Vec<Entity>>,
    mut newly_visible: Local<Vec<Entity>>,
    mut already_visible: Local<Vec<Entity>>,
) {
    orphaned_entities.clear();
    orphaned_entities.extend(orphaned.iter());
    orphaned_entities.sort_unstable();

    all_visible.clear();
    for visible_entities in &visible_entities_query {
        all_visible.append(&mut visible_entities.entities.clone());
    }
    all_visible.sort_unstable();

    already_visible.retain(|entity| all_visible.binary_search(entity).is_ok());

    newly_visible.clear();
    for entity in &all_visible {
        if already_visible.binary_search(entity).is_ok() {
            continue;
        }
        newly_visible.push(*entity);
        already_visible.push(*entity);
    }

    root_query.par_iter_mut().for_each_mut(
        |(entity, children, transform, mut global_transform)| {
            let changed = transform.is_changed() || global_transform.is_added() || newly_visible.binary_search(&entity).is_ok() || orphaned_entities.binary_search(&entity).is_ok();
            if changed {
                *global_transform = GlobalTransform::from(*transform);
            }

            if all_visible.binary_search(&entity).is_err() {
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
        (With<Parent>, With<Cocos2dAtlasSprite>),
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

        changed |= transform.is_changed() || global_transform.is_added();
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
    mut orphaned: RemovedComponents<Parent>,
    transform_query: Query<
        (Ref<Transform>, &mut GlobalTransform, Option<&Children>),
        (With<Parent>, Without<Cocos2dAtlasSprite>),
    >,
    parent_query: Query<(Entity, Ref<Parent>), Without<Cocos2dAtlasSprite>>,
    mut orphaned_entities: Local<Vec<Entity>>,
) {
    orphaned_entities.clear();
    orphaned_entities.extend(orphaned.iter());
    orphaned_entities.sort_unstable();
    root_query.par_iter_mut().for_each_mut(
        |(entity, children, transform, mut global_transform)| {
            let changed = transform.is_changed() || global_transform.is_added() || orphaned_entities.binary_search(&entity).is_ok();
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

        changed |= transform.is_changed() || global_transform.is_added();
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
