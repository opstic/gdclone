use bevy::hierarchy::Parent;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{
    Changed, Component, DetectChangesMut, Entity, Query, Resource, Without, World,
};
use bevy::utils::default;
use indexmap::IndexMap;
use smallvec::SmallVec;

use crate::level::transform::Transform2d;
use crate::level::trigger::Trigger;
use crate::utils::U64Hash;

#[derive(Resource)]
pub(crate) struct GlobalGroups(pub(crate) SmallVec<[Entity; 1000]>);

impl Default for GlobalGroups {
    fn default() -> Self {
        Self(SmallVec::from_buf([Entity::PLACEHOLDER; 1000]))
    }
}

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
    pub(crate) translation_delta: Vec2,
    pub(crate) rotate_around: Option<(Entity, f32, bool)>,
    pub(crate) rotation: f32,
}

#[derive(Component)]
pub(crate) struct ObjectGroups {
    pub(crate) groups: Vec<(u64, Entity, f32, bool)>,
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
    // global_group_query
    //     .par_iter_mut()
    //     .for_each(|mut global_group| {
    //         let global_group = global_group.bypass_change_detection();
    //         global_group.translation_delta = Vec2::ZERO;
    //         global_group.rotate_around = None;
    //         global_group.rotation = Quat::IDENTITY;
    //     })
    for mut global_group in &mut global_group_query {
        let global_group = global_group.bypass_change_detection();
        global_group.translation_delta = Vec2::ZERO;
        global_group.rotate_around = None;
        global_group.rotation = 0.;
    }
}

pub(crate) fn apply_group_delta(
    mut objects: Query<&mut Transform2d, (Without<Parent>, Without<Trigger>)>,
    groups: Query<(&GlobalGroup, &GlobalGroupDeltas), Changed<GlobalGroupDeltas>>,
) {
    for (group, group_deltas) in &groups {
        let mut iter = objects.iter_many_mut(&group.root_entities);

        let translation_delta = group_deltas.translation_delta.extend(0.);

        while let Some(mut transform) = iter.fetch_next() {
            transform.translation += translation_delta;
            transform.angle += group_deltas.rotation;
        }
    }

    for (group, group_deltas) in &groups {
        let Some((center_entity, rotation, lock_rotation)) = group_deltas.rotate_around else {
            continue;
        };

        let cos_sin = Vec2::from_angle(rotation);

        let Ok(center_transform) = objects.get(center_entity) else {
            continue;
        };

        let center_transform = center_transform.translation.xy();

        let mut iter = objects.iter_many_mut(&group.root_entities);

        if !lock_rotation {
            while let Some(mut transform) = iter.fetch_next() {
                transform.translate_around_cos_sin(center_transform, cos_sin);
                transform.angle += rotation
            }
        } else {
            while let Some(mut transform) = iter.fetch_next() {
                transform.translate_around_cos_sin(center_transform, cos_sin);
            }
        }
    }
}

pub(crate) fn spawn_groups(
    world: &mut World,
    global_groups_data: IndexMap<u64, Vec<Entity>, U64Hash>,
) {
    let mut global_groups = GlobalGroups::default();

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

        if group >= global_groups.0.len() as u64 {
            global_groups
                .0
                .resize((group + 1) as usize, Entity::PLACEHOLDER);
        }

        global_groups.0[group as usize] = group_entity;
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
                if !group_enabled {
                    object_groups_calculated.enabled = false;
                    break;
                }
                object_groups_calculated.opacity *= group_opacity;
            }
        });
}
