use std::ops::Range;

use bevy::math::{IVec2, Vec2, Vec3Swizzles};
use bevy::prelude::{
    Changed, Component, DetectChangesMut, Entity, Query, ResMut, Resource, Transform,
};
use dashmap::{DashMap, DashSet};

use crate::utils::U64Hash;

#[derive(Default, Resource)]
pub(crate) struct GlobalSections(pub(crate) DashMap<IVec2, DashSet<Entity, U64Hash>>);

#[derive(Default, Resource)]
pub(crate) struct VisibleGlobalSections {
    pub(crate) x: Range<i32>,
    pub(crate) y: Range<i32>,
}

#[derive(Component)]
pub(crate) struct Section {
    pub(crate) current: IVec2,
    pub(crate) old: Option<IVec2>,
}

impl Section {
    const SIZE: f32 = 200.;

    #[inline(always)]
    pub(crate) fn index_from_pos(pos: Vec2) -> IVec2 {
        IVec2::new((pos.x / Self::SIZE) as i32, (pos.y / Self::SIZE) as i32)
    }
}

pub(crate) fn update_entity_section(
    mut entities: Query<(&Transform, &mut Section), Changed<Transform>>,
) {
    entities
        .par_iter_mut()
        .for_each(|(transform, mut section)| {
            let new_section = Section::index_from_pos(transform.translation.xy());
            if section.current != new_section {
                section.old = Some(section.current);
                section.current = new_section;
            }
        });
}

pub(crate) fn update_global_sections(
    global_sections: ResMut<GlobalSections>,
    mut section_changed_entities: Query<(Entity, &mut Section), Changed<Section>>,
) {
    section_changed_entities
        .par_iter_mut()
        .for_each(|(entity, mut section)| {
            let section = section.bypass_change_detection();
            if let Some(old) = section.old {
                if let Some(global_section) = global_sections.0.get(&old) {
                    global_section.remove(&entity);
                }
                section.old = None;
            }

            let Some(global_section) = global_sections.0.get(&section.current) else {
                let global_section_entry = global_sections.0.entry(section.current);
                global_section_entry.or_default().insert(entity);
                return;
            };

            global_section.insert(entity);
        });
}
