use bevy::math::Vec2;
use bevy::prelude::{Changed, Component, DetectChangesMut, Entity, Query, Resource, World};
use bevy::utils::default;
use dashmap::DashMap;
use indexmap::{IndexMap, IndexSet};

use crate::utils::U64Hash;

#[derive(Default, Resource)]
pub(crate) struct GlobalGroups(pub(crate) DashMap<u64, Entity, U64Hash>);

#[derive(Component)]
pub(crate) struct GlobalGroup {
    id: u64,
    entities: IndexSet<Entity, U64Hash>,
    pub(crate) opacity: f32,
    pub(crate) activated: bool,
}

impl Default for GlobalGroup {
    fn default() -> Self {
        Self {
            id: u64::MAX,
            entities: IndexSet::with_hasher(U64Hash),
            opacity: 1.,
            activated: true,
        }
    }
}

#[derive(Default, Component)]
pub(crate) struct GlobalGroupDeltas {
    deltas: Vec<TransformDelta>,
    rotation: f32,
}

#[derive(Component)]
pub(crate) struct ObjectGroups {
    groups: Vec<(u64, Entity, f32, bool)>,
}

pub(crate) enum TransformDelta {
    Translate { delta: Vec2 },
    RotateAround { center: Vec2, degrees: f32 },
}

#[derive(Component)]
pub(crate) struct ObjectGroupsCalculated {
    pub(crate) opacity: f32,
    pub(crate) enabled: bool,
}

impl Default for ObjectGroupsCalculated {
    fn default() -> Self {
        Self {
            opacity: 1.,
            enabled: true,
        }
    }
}

pub(crate) fn clear_group_delta(
    mut global_group_query: Query<&mut GlobalGroupDeltas, Changed<GlobalGroupDeltas>>,
) {
    global_group_query
        .par_iter_mut()
        .for_each(|mut global_group| {
            let global_group = global_group.bypass_change_detection();
            global_group.deltas.clear();
            global_group.rotation = 0.;
        })
}

pub(crate) fn spawn_groups(
    world: &mut World,
    global_groups_data: IndexMap<u64, Vec<Entity>, U64Hash>,
) {
    let global_groups = GlobalGroups::default();

    for (group, entities) in global_groups_data {
        let group_entity = world
            .spawn((
                GlobalGroup {
                    id: group,
                    entities: IndexSet::from_iter(entities.iter().copied()),
                    ..default()
                },
                GlobalGroupDeltas::default(),
            ))
            .id();

        for entity in entities {
            let mut world_entity_mut = world.entity_mut(entity);
            if let Some(mut object_groups) = world_entity_mut.get_mut::<ObjectGroups>() {
                object_groups.groups.push((group, group_entity, 1., true));
            } else {
                world_entity_mut.insert(ObjectGroups {
                    groups: vec![(group, group_entity, 1., true)],
                });
            }
        }

        global_groups.0.insert(group, group_entity);
    }

    world.insert_resource(global_groups);
}

pub(crate) fn update_object_group(
    mut objects: Query<&mut ObjectGroups>,
    groups: Query<&GlobalGroup, Changed<GlobalGroup>>,
) {
    for global_group in &groups {
        let mut iter = objects.iter_many_mut(&global_group.entities);

        while let Some(mut object_groups) = iter.fetch_next() {
            let Some((_, _, group_opacity, group_enabled)) = object_groups
                .groups
                .iter_mut()
                .find(|(id, _, _, _)| *id == global_group.id)
            else {
                panic!("Object doesn't have group in list??");
            };
            *group_opacity = global_group.opacity;
            *group_enabled = global_group.activated;
            // for (_, group_entity, group_opacity, group_enabled) in &mut object_groups.groups {
            //     let Ok(global_group) = groups.get(*group_entity) else {
            //         continue;
            //     };
            //
            //     *group_opacity = global_group.opacity;
            //     *group_enabled = global_group.enabled;
            // }
        }
    }
}

pub(crate) fn update_object_group_calculated(
    mut objects: Query<(&ObjectGroups, &mut ObjectGroupsCalculated), Changed<ObjectGroups>>,
) {
    objects
        .par_iter_mut()
        .for_each(|(object_groups, mut object_groups_calculated)| {
            object_groups_calculated.opacity = 1.;
            object_groups_calculated.enabled = true;
            for (_, _, group_opacity, group_enabled) in &object_groups.groups {
                object_groups_calculated.opacity *= group_opacity;
                object_groups_calculated.enabled &= *group_enabled;
            }
        });
}
