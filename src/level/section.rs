use std::hash::Hash;
use std::ops::Range;

use bevy::ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy::hierarchy::{Children, Parent};
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{
    Changed, Component, Entity, Mut, Query, ResMut, Resource, Transform, With, Without,
};
use dashmap::DashMap;
use indexmap::IndexSet;

use crate::utils::U64Hash;

#[derive(Default, Resource)]
pub(crate) struct GlobalSections(
    pub(crate) DashMap<SectionIndex, IndexSet<Entity, U64Hash>, U64Hash>,
);

#[derive(Default, Resource)]
pub(crate) struct VisibleGlobalSections {
    pub(crate) x: Range<i32>,
    pub(crate) y: Range<i32>,
}

#[derive(Copy, Clone, Component)]
pub(crate) struct Section {
    pub(crate) current: SectionIndex,
    pub(crate) old: SectionIndex,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct SectionIndex {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl SectionIndex {
    const SIZE: f32 = 200.;

    #[inline]
    pub(crate) fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub(crate) fn from_pos(pos: Vec2) -> Self {
        Self {
            x: (pos.x * (1. / Self::SIZE)) as i32,
            y: (pos.y * (1. / Self::SIZE)) as i32,
        }
    }
}

impl Hash for SectionIndex {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ((unsafe { std::mem::transmute::<i32, u32>(self.x) } as u64) << 32
            | unsafe { std::mem::transmute::<i32, u32>(self.y) } as u64)
            .hash(state)
    }
}

pub(crate) fn update_entity_section(
    mut entities: Query<(&Transform, &mut Section), (Without<Parent>, Changed<Transform>)>,
) {
    entities
        .par_iter_mut()
        .for_each(|(transform, mut section)| {
            let new_section = SectionIndex::from_pos(transform.translation.xy());
            if section.current != new_section {
                section.old = section.current;
                section.current = new_section;
            }
        });
}

pub(crate) fn propagate_section_change(
    section_changed_entities: Query<(&Section, &Children), (Changed<Section>, Without<Parent>)>,
    children_query: Query<(&mut Section, Option<&Children>), With<Parent>>,
) {
    section_changed_entities
        .par_iter()
        .for_each(|(section, children)| unsafe {
            propagate_section_recursive(children, &children_query, section)
        });
}

unsafe fn propagate_section_recursive<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery>(
    children: &Children,
    children_query: &'w Query<'w, 's, Q, F>,
    parent_section: &Section,
) where
    Q: WorldQuery<Item<'w> = (Mut<'w, Section>, Option<&'w Children>)>,
{
    for child_entity in children {
        let Ok((mut section, children)) = children_query.get_unchecked(*child_entity) else {
            continue;
        };

        *section = *parent_section;

        let Some(children) = children else {
            continue;
        };

        propagate_section_recursive(children, children_query, parent_section);
    }
}

pub(crate) fn update_global_sections(
    global_sections: ResMut<GlobalSections>,
    section_changed_entities: Query<(Entity, &Section), Changed<Section>>,
) {
    section_changed_entities
        .par_iter()
        .for_each(|(entity, section)| {
            if let Some(mut global_section) = global_sections.0.get_mut(&section.old) {
                global_section.remove(&entity);
            }

            let global_section_entry = global_sections.0.entry(section.current);
            let mut global_section = global_section_entry.or_default();
            global_section.insert(entity);
        });
}
