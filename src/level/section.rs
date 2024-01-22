use std::hash::Hash;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy::hierarchy::{Children, Parent};
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::prelude::{Changed, Component, Entity, Mut, Query, Res, ResMut, Resource, With, Without};
use bevy::tasks::ComputeTaskPool;
use bevy::utils::syncunsafecell::SyncUnsafeCell;
use dashmap::DashMap;
use indexmap::IndexSet;

use crate::level::transform::Transform2d;
use crate::utils::{dashmap_get_dirty_mut, U64Hash};

#[derive(Default, Resource)]
pub(crate) struct GlobalSections {
    pub(crate) sections: DashMap<SectionIndex, IndexSet<Entity, U64Hash>, U64Hash>,
    pub(crate) visible: (
        AtomicUsize,
        SyncUnsafeCell<Vec<MaybeUninit<&'static IndexSet<Entity, U64Hash>>>>,
    ),
}

#[derive(Default, Resource)]
pub(crate) struct VisibleGlobalSections {
    pub(crate) x: Range<i32>,
    pub(crate) y: Range<i32>,
}

#[derive(Copy, Clone, Component, Default)]
pub(crate) struct Section {
    pub(crate) current: SectionIndex,
    pub(crate) old: SectionIndex,
}

impl Section {
    pub(crate) fn from_section_index(index: SectionIndex) -> Section {
        Section {
            current: index,
            old: SectionIndex::new(0, 0),
        }
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) struct SectionIndex {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl SectionIndex {
    const SIZE: f32 = 200.;
    const INVERSE: f32 = 1. / Self::SIZE;

    #[inline]
    pub(crate) fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub(crate) fn from_pos(pos: Vec2) -> Self {
        Self {
            x: (pos.x * Self::INVERSE) as i32,
            y: (pos.y * Self::INVERSE) as i32,
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

pub(crate) fn update_visible_sections(
    mut global_sections: ResMut<GlobalSections>,
    visible_global_sections: Res<VisibleGlobalSections>,
) {
    let section_len = visible_global_sections.x.len() * visible_global_sections.y.len();

    global_sections.visible.0.store(0, Ordering::Relaxed);
    let visible_sections_len = global_sections.visible.1.get_mut().len();
    if visible_sections_len < section_len {
        global_sections
            .visible
            .1
            .get_mut()
            .resize(section_len, MaybeUninit::uninit());
    }
    let compute_task_pool = ComputeTaskPool::get();

    let x_list = visible_global_sections.x.clone().collect::<Vec<_>>();
    let global_sections = unsafe { &*((&*global_sections) as *const GlobalSections) };
    compute_task_pool.scope(|scope| {
        let visible_global_sections = &visible_global_sections;

        let chunk_size = (x_list.len() / compute_task_pool.thread_num()).max(1);
        for chunk in x_list.chunks(chunk_size) {
            scope.spawn(async move {
                let a = &global_sections.visible.1;
                let sections_to_extract = unsafe { &mut *a.get() };
                for x in chunk {
                    for y in visible_global_sections.y.clone() {
                        let section_index = SectionIndex::new(*x, y);

                        let Some(global_section) = (unsafe {
                            dashmap_get_dirty_mut(&section_index, &global_sections.sections)
                        }) else {
                            continue;
                        };

                        // Improve access pattern by sorting the section
                        global_section.sort_unstable();

                        let index = global_sections.visible.0.fetch_add(1, Ordering::Relaxed);
                        sections_to_extract[index] = MaybeUninit::new(global_section);
                    }
                }
            });
        }
    });
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
            let new_section = SectionIndex::from_pos(transform.translation.xy());
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
    global_sections: ResMut<GlobalSections>,
    section_changed_entities: Query<(Entity, &Section), Changed<Section>>,
) {
    section_changed_entities
        .par_iter()
        .for_each(|(entity, section)| {
            if let Some(mut global_section) = global_sections.sections.get_mut(&section.old) {
                global_section.remove(&entity);
            }

            let global_section_entry = global_sections.sections.entry(section.current);
            let mut global_section = global_section_entry.or_default();
            global_section.insert(entity);
        });
}
