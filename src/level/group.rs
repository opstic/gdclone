use crate::utils::U64Hash;
use bevy::math::Vec2;
use bevy::prelude::{Changed, Component, DetectChangesMut, Entity, Query, Resource, World};
use bevy::utils::default;
use dashmap::DashMap;
use indexmap::{IndexMap, IndexSet};

#[derive(Default, Resource)]
pub(crate) struct GlobalGroups(pub(crate) DashMap<u64, Entity, U64Hash>);

#[derive(Component)]
pub(crate) struct GlobalGroup {
    entities: IndexSet<Entity, U64Hash>,
    opacity: f32,
    enabled: bool,
    deltas: (Vec<TransformDelta>, f32),
}

impl Default for GlobalGroup {
    fn default() -> Self {
        Self {
            entities: IndexSet::with_hasher(U64Hash),
            opacity: 1.,
            enabled: true,
            deltas: (Vec::new(), 0.),
        }
    }
}

#[derive(Component)]
pub(crate) struct ObjectGroups {
    groups: Vec<(u64, Entity)>,
}

pub(crate) enum TransformDelta {
    Translate { delta: Vec2 },
    RotateAround { center: Vec2, degrees: f32 },
}

pub(crate) fn clear_group_delta(
    mut global_group_query: Query<&mut GlobalGroup, Changed<GlobalGroup>>,
) {
    global_group_query
        .par_iter_mut()
        .for_each(|mut global_group| {
            let global_group = global_group.bypass_change_detection();
            global_group.deltas.0.clear();
            global_group.deltas.1 = 0.;
        })
}

pub(crate) fn spawn_groups(
    world: &mut World,
    global_groups_data: IndexMap<u64, Vec<Entity>, U64Hash>,
) {
    let global_groups = GlobalGroups::default();

    for (group, entities) in global_groups_data {
        let group_entity = world
            .spawn(GlobalGroup {
                entities: IndexSet::from_iter(entities.iter().copied()),
                ..default()
            })
            .id();

        for entity in entities {
            let mut world_entity_mut = world.entity_mut(entity);
            if let Some(mut object_groups) = world_entity_mut.get_mut::<ObjectGroups>() {
                object_groups.groups.push((group, group_entity));
            } else {
                world_entity_mut.insert(ObjectGroups {
                    groups: vec![(group, group_entity)],
                });
            }
        }

        global_groups.0.insert(group, group_entity);
    }

    world.insert_resource(global_groups);
}
