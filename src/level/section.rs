use std::ops::Range;
use std::sync::Mutex;

use bevy::ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy::hierarchy::{Children, Parent};
use bevy::prelude::{Changed, Component, Entity, Mut, Query, ResMut, Resource, With, Without};
use indexmap::IndexSet;
use smallvec::SmallVec;

use crate::level::transform::Transform2d;
use crate::utils::{section_index_from_x, U64Hash};

#[derive(Default, Resource)]
pub(crate) struct GlobalSections {
    pub(crate) sections: SmallVec<[IndexSet<Entity, U64Hash>; 4000]>,
    pub(crate) visible: Range<usize>,
}

#[derive(Copy, Clone, Component, Default)]
pub(crate) struct Section {
    pub(crate) current: u32,
    pub(crate) old: u32,
}

impl Section {
    pub(crate) fn from_section_index(index: u32) -> Section {
        Section {
            current: index,
            old: 0,
        }
    }
}

pub(crate) fn update_entity_section(
    mut entities: Query<
        (&Transform2d, &mut Section, Option<&Children>),
        (Without<Parent>, Changed<Transform2d>),
    >,
    children_query: Query<(&mut Section, Option<&Children>), With<Parent>>,
) {
    entities
        .par_iter_mut()
        .for_each(|(transform, mut section, children)| {
            let new_section = section_index_from_x(transform.translation.x);
            if section.current == new_section {
                return;
            }

            section.old = section.current;
            section.current = new_section;

            let Some(children) = children else {
                return;
            };

            unsafe { propagate_section_recursive(children, &children_query, &section) }
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
    mut global_sections: ResMut<GlobalSections>,
    section_changed_entities: Query<(Entity, &Section), Changed<Section>>,
) {
    let sections_lock = Mutex::new(&mut global_sections.sections);
    section_changed_entities
        .par_iter()
        .for_each(|(entity, section)| {
            let mut global_sections = sections_lock.lock().unwrap();
            global_sections[section.old as usize].remove(&entity);

            if section.current >= global_sections.len() as u32 {
                global_sections.resize(
                    (section.current + 1) as usize,
                    IndexSet::with_capacity_and_hasher(1000, U64Hash),
                );
            }

            global_sections[section.current as usize].insert(entity);
        });

    global_sections.visible.start = global_sections
        .visible
        .start
        .min(global_sections.sections.len());
    global_sections.visible.end = global_sections
        .visible
        .end
        .min(global_sections.sections.len());
}
