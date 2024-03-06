use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::hierarchy::{Children, Parent};
use bevy::prelude::{
    Changed, Component, Entity, Local, Mut, Query, ResMut, Resource, With, Without,
};
use bevy::utils::syncunsafecell::SyncUnsafeCell;
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

pub(crate) fn update_sections(
    mut global_sections: ResMut<GlobalSections>,
    all_entities: Query<(), Changed<Transform2d>>,
    mut entities: Query<
        (Entity, &Transform2d, &mut Section, Option<&Children>),
        (Without<Parent>, Changed<Transform2d>),
    >,
    children_query: Query<(&mut Section, Option<&Children>), With<Parent>>,
    mut changed_entities: Local<(AtomicUsize, SyncUnsafeCell<Vec<(u32, u32, Entity)>>)>,
) {
    let (lower_bound, upper_bound) = all_entities.iter().size_hint();
    changed_entities.1.get_mut().resize(
        upper_bound.unwrap_or(lower_bound),
        (0, 0, Entity::PLACEHOLDER),
    );
    changed_entities.0.store(0, Ordering::Relaxed);

    entities
        .par_iter_mut()
        .for_each(|(entity, transform, mut section, children)| {
            let new_section = section_index_from_x(transform.translation.x);
            if section.current == new_section {
                return;
            }

            section.old = section.current;
            section.current = new_section;

            let index = changed_entities.0.fetch_add(1, Ordering::Relaxed);
            let array = unsafe { &mut *changed_entities.1.get() };
            array[index] = (section.old, section.current, entity);

            let Some(children) = children else {
                return;
            };

            unsafe {
                propagate_section_recursive(children, &children_query, &section, &changed_entities)
            }
        });

    let length = changed_entities.0.load(Ordering::Relaxed);
    for (old, new, entity) in &changed_entities.1.get_mut()[..length] {
        if *new >= global_sections.sections.len() as u32 {
            global_sections.sections.resize(
                (*new + 1) as usize,
                IndexSet::with_capacity_and_hasher(1000, U64Hash),
            );
        }

        global_sections.sections[*old as usize].swap_remove(entity);
        global_sections.sections[*new as usize].insert(*entity);
    }
}

unsafe fn propagate_section_recursive<'w, 's, D: QueryData, F: QueryFilter>(
    children: &Children,
    children_query: &'w Query<'w, 's, D, F>,
    parent_section: &Section,
    changed_entities: &(AtomicUsize, SyncUnsafeCell<Vec<(u32, u32, Entity)>>),
) where
    D: QueryData<Item<'w> = (Mut<'w, Section>, Option<&'w Children>)>,
{
    for child_entity in children {
        let Ok((mut section, children)) = children_query.get_unchecked(*child_entity) else {
            continue;
        };

        *section = *parent_section;

        let index = changed_entities.0.fetch_add(1, Ordering::Relaxed);
        let array = unsafe { &mut *changed_entities.1.get() };
        array[index] = (section.old, section.current, *child_entity);

        let Some(children) = children else {
            continue;
        };

        propagate_section_recursive(children, children_query, parent_section, changed_entities);
    }
}

pub(crate) fn limit_sections(mut global_sections: ResMut<GlobalSections>) {
    global_sections.visible.start = global_sections
        .visible
        .start
        .min(global_sections.sections.len());
    global_sections.visible.end = global_sections
        .visible
        .end
        .min(global_sections.sections.len());
}
