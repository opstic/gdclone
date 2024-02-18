use bevy::hierarchy::Parent;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{
    Changed, Component, DetectChangesMut, Entity, Or, Query, Resource, Without, World,
};
use bevy::utils::default;
use indexmap::IndexMap;
use smallvec::SmallVec;

use crate::level::color::Pulses;
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
    pub(crate) archetypes: SmallVec<[Entity; 250]>,
    pub(crate) opacity: f32,
    pub(crate) enabled: bool,
}

impl Default for GlobalGroup {
    fn default() -> Self {
        Self {
            id: u64::MAX,
            entities: Vec::with_capacity(1000),
            root_entities: Vec::with_capacity(1000),
            archetypes: SmallVec::from_buf([Entity::PLACEHOLDER; 250]),
            opacity: 1.,
            enabled: true,
        }
    }
}

#[derive(Default, Component)]
pub(crate) struct GlobalGroupDeltas {
    pub(crate) translation_delta: Vec2,
    pub(crate) rotation: RotationKind,
}

pub(crate) enum RotationKind {
    Around(Entity, f32, bool),
    Angle(f32),
}

impl Default for RotationKind {
    fn default() -> Self {
        Self::Angle(0.)
    }
}

#[derive(Component, Default)]
pub(crate) struct GroupArchetype {
    pub(crate) groups: Vec<(u64, f32, bool)>,
    pub(crate) want_to_enable: bool,
}

#[derive(Component)]
pub(crate) struct GroupArchetypeCalculated {
    pub(crate) opacity: f32,
    pub(crate) enabled: bool,
}

impl Default for GroupArchetypeCalculated {
    fn default() -> Self {
        Self {
            opacity: 1.,
            enabled: true,
        }
    }
}

#[derive(Component)]
pub(crate) struct ObjectGroups {
    pub(crate) groups: Vec<u64>,
    pub(crate) archetype_entity: Entity,
}

pub(crate) fn clear_group_delta(
    mut global_group_query: Query<&mut GlobalGroupDeltas, Changed<GlobalGroupDeltas>>,
) {
    for mut global_group in &mut global_group_query {
        let global_group = global_group.bypass_change_detection();
        global_group.translation_delta = Vec2::ZERO;
        global_group.rotation = RotationKind::Angle(0.);
    }
}

pub(crate) fn apply_group_delta(
    mut objects: Query<&mut Transform2d, (Without<Parent>, Without<Trigger>)>,
    groups: Query<(&GlobalGroup, &GlobalGroupDeltas), Changed<GlobalGroupDeltas>>,
) {
    for (group, group_deltas) in &groups {
        let mut iter = objects.iter_many_mut(&group.root_entities);

        let translation_delta = group_deltas.translation_delta.extend(0.);

        let rotation = match group_deltas.rotation {
            RotationKind::Angle(rotation) => rotation,
            _ => 0.,
        };

        while let Some(mut transform) = iter.fetch_next() {
            transform.translation += translation_delta;
            transform.angle += rotation;
        }
    }

    for (group, group_deltas) in &groups {
        let RotationKind::Around(center_entity, rotation, lock_rotation) = group_deltas.rotation
        else {
            continue;
        };

        let Ok(center_transform) = objects.get(center_entity) else {
            continue;
        };

        let center_transform = center_transform.translation.xy();

        let cos_sin = Vec2::from_angle(rotation);

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
    global_groups_data: IndexMap<u64, (Vec<Entity>, Vec<Entity>), U64Hash>,
    group_archetypes: IndexMap<Vec<u64>, Vec<Entity>>,
) {
    let mut global_groups = GlobalGroups::default();

    let mut group_archetype_group: IndexMap<u64, Vec<Entity>, U64Hash> =
        IndexMap::with_capacity_and_hasher(global_groups_data.len(), U64Hash);

    for (groups, entities) in group_archetypes {
        let archetype_entity = world
            .spawn((
                GroupArchetype {
                    groups: groups
                        .iter()
                        .map(|group_id| (*group_id, 1., true))
                        .collect(),
                    ..default()
                },
                GroupArchetypeCalculated::default(),
                Pulses::default(),
            ))
            .id();

        for entity in entities {
            let mut world_entity_mut = world.entity_mut(entity);
            world_entity_mut.insert(ObjectGroups {
                groups: groups.to_vec(),
                archetype_entity,
            });
        }

        for group in groups {
            group_archetype_group
                .entry(group)
                .or_default()
                .push(archetype_entity);
        }
    }

    for (group, (root_entities, entities)) in global_groups_data {
        let group_entity = world
            .spawn((
                GlobalGroup {
                    id: group,
                    entities,
                    root_entities,
                    archetypes: (*group_archetype_group.get(&group).unwrap()).clone().into(),
                    ..default()
                },
                GlobalGroupDeltas::default(),
                Pulses::default(),
            ))
            .id();

        if group >= global_groups.0.len() as u64 {
            global_groups
                .0
                .resize((group + 1) as usize, Entity::PLACEHOLDER);
        }

        global_groups.0[group as usize] = group_entity;
    }

    world.insert_resource(global_groups);
}

pub(crate) fn update_group_archetype(
    mut group_archetypes: Query<(
        &mut GroupArchetype,
        &mut GroupArchetypeCalculated,
        &mut Pulses,
    )>,
    groups: Query<
        (&GlobalGroup, &Pulses),
        (
            Or<(Changed<GlobalGroup>, Changed<Pulses>)>,
            Without<GroupArchetype>,
        ),
    >,
) {
    for (global_group, group_pulses) in &groups {
        let mut iter = group_archetypes.iter_many_mut(&global_group.archetypes);

        while let Some((
            mut group_archetype,
            mut group_archetype_calculated,
            mut group_archetype_pulses,
        )) = iter.fetch_next()
        {
            let Some((_, group_opacity, group_enabled)) = group_archetype
                .groups
                .iter_mut()
                .find(|(id, _, _)| *id == global_group.id)
            else {
                panic!("Archetype doesn't have group in list??");
            };
            *group_opacity = global_group.opacity;
            *group_enabled = global_group.enabled;

            if !global_group.enabled {
                group_archetype_calculated.enabled = false;
            } else {
                group_archetype.want_to_enable = true;
            }

            if group_pulses.pulses.is_empty() {
                continue;
            }

            if group_pulses.pulses[0].0 == 1. {
                group_archetype_pulses.pulses.clear();
            }

            group_archetype_pulses
                .pulses
                .extend_from_slice(&group_pulses.pulses)
        }
    }
}

pub(crate) fn update_group_archetype_calculated(
    mut group_archetypes: Query<
        (&mut GroupArchetype, &mut GroupArchetypeCalculated),
        Changed<GroupArchetype>,
    >,
) {
    group_archetypes.par_iter_mut().for_each(
        |(mut group_archetype, mut group_archetype_calculated)| {
            if !group_archetype_calculated.enabled && !group_archetype.want_to_enable {
                return;
            }

            group_archetype_calculated.opacity = 1.;
            for (_, group_opacity, group_enabled) in &group_archetype.groups {
                if !group_enabled {
                    group_archetype_calculated.enabled = false;
                    group_archetype.want_to_enable = false;
                    return;
                }
                group_archetype_calculated.opacity *= group_opacity;
            }

            group_archetype_calculated.enabled = true;
            group_archetype.want_to_enable = false;
        },
    );
}
