use bevy::hierarchy::Parent;
use bevy::math::{Quat, Vec2};
use bevy::prelude::{
    Changed, Component, DetectChangesMut, Entity, Query, Resource, Transform, Without, World,
};
use bevy::utils::default;
use dashmap::DashMap;
use indexmap::IndexMap;

use crate::level::trigger::Trigger;
use crate::utils::U64Hash;

#[derive(Default, Resource)]
pub(crate) struct GlobalGroups(pub(crate) DashMap<u64, Entity, U64Hash>);

#[derive(Component)]
pub(crate) struct GlobalGroup {
    id: u64,
    pub(crate) entities: Vec<Entity>,
    pub(crate) root_entities: Vec<Entity>,
    pub(crate) opacity: f32,
    pub(crate) enabled: bool,
}

impl Default for GlobalGroup {
    fn default() -> Self {
        Self {
            id: u64::MAX,
            entities: Vec::with_capacity(1000),
            root_entities: Vec::with_capacity(1000),
            opacity: 1.,
            enabled: true,
        }
    }
}

#[derive(Default, Component)]
pub(crate) struct GlobalGroupDeltas {
    pub(crate) deltas: Vec<TransformDelta>,
    pub(crate) rotation: Quat,
}

#[derive(Component)]
pub(crate) struct ObjectGroups {
    pub(crate) groups: Vec<(u64, Entity, f32, bool)>,
}

#[derive(Debug)]
pub(crate) enum TransformDelta {
    Translate {
        delta: Vec2,
    },
    RotateAround {
        center: Vec2,
        rotation: Quat,
        lock_rotation: bool,
    },
}

impl TransformDelta {
    pub(crate) fn apply(&self, transform: &mut Transform) {
        match self {
            TransformDelta::Translate { delta } => transform.translation += delta.extend(0.),
            TransformDelta::RotateAround {
                center,
                rotation,
                lock_rotation,
            } => {
                transform.translate_around(center.extend(0.), *rotation);
                if !lock_rotation {
                    transform.rotate(*rotation);
                }
            }
        }
    }
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
            global_group.rotation = Quat::IDENTITY;
        })
}

pub(crate) fn apply_group_delta(
    mut objects: Query<&mut Transform, (Without<Parent>, Without<Trigger>)>,
    groups: Query<(&GlobalGroup, &GlobalGroupDeltas), Changed<GlobalGroupDeltas>>,
) {
    for (group, group_deltas) in &groups {
        let mut iter = objects.iter_many_mut(&group.entities);

        while let Some(mut transform) = iter.fetch_next() {
            for delta in &group_deltas.deltas {
                delta.apply(&mut transform);
            }
        }
    }
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
                    entities: entities.clone(),
                    root_entities: entities
                        .iter()
                        .filter(|entity| !world.entity(**entity).contains::<Parent>())
                        .copied()
                        .collect(),
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
            *group_enabled = global_group.enabled;
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
