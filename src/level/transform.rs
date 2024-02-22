use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::hierarchy::{Children, Parent};
use bevy::math::{Affine2, Mat2, Vec2, Vec3, Vec3Swizzles};
use bevy::prelude::{Component, DetectChanges, Mut, Query, Ref, Res, With, Without};
use bevy::tasks::ComputeTaskPool;

use crate::level::collision::{GlobalHitbox, Hitbox};
use crate::level::section::GlobalSections;

#[derive(Clone, Component, Copy)]
pub(crate) struct Transform2d {
    pub(crate) translation: Vec3,
    pub(crate) angle: f32,
    pub(crate) shear: Vec2,
    pub(crate) scale: Vec2,
}

impl Default for Transform2d {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            angle: 0.,
            shear: Vec2::ZERO,
            scale: Vec2::ONE,
        }
    }
}

impl Transform2d {
    #[inline]
    pub(crate) fn translate_around_cos_sin(&mut self, point: Vec2, angle: Vec2) {
        self.translation =
            (point + (self.translation.xy() - point).rotate(angle)).extend(self.translation.z)
    }
}

#[derive(Clone, Component, Copy, Default)]
pub(crate) struct GlobalTransform2d {
    affine: Affine2,
    z: f32,
}

impl From<Transform2d> for GlobalTransform2d {
    fn from(transform: Transform2d) -> Self {
        let mut affine = Affine2::from_scale_angle_translation(
            transform.scale,
            transform.angle,
            transform.translation.xy(),
        );

        if transform.shear != Vec2::ZERO {
            affine.matrix2 *= Mat2::from_cols_array_2d(&[
                [1., transform.shear.x.tan().copysign(transform.scale.x)],
                [transform.shear.y.tan().copysign(transform.scale.y), 1.],
            ]);
        }

        Self {
            affine,
            z: transform.translation.z,
        }
    }
}

impl GlobalTransform2d {
    #[inline]
    pub(crate) fn mul_transform(&self, transform: Transform2d) -> Self {
        let rhs = GlobalTransform2d::from(transform);
        Self {
            affine: self.affine * rhs.affine,
            z: self.z + rhs.z,
        }
    }

    #[inline]
    pub(crate) fn affine(&self) -> Affine2 {
        self.affine
    }

    #[inline]
    pub(crate) fn z(&self) -> f32 {
        self.z
    }
}

pub(crate) fn update_transform(
    global_sections: Res<GlobalSections>,
    object_query: Query<
        (
            Ref<Transform2d>,
            &mut GlobalTransform2d,
            Option<(&Hitbox, &mut GlobalHitbox)>,
            Option<&Children>,
        ),
        Without<Parent>,
    >,
    children_query: Query<(&Transform2d, &mut GlobalTransform2d, Option<&Children>), With<Parent>>,
) {
    let sections_to_update = &global_sections.sections[global_sections.visible.clone()];

    let compute_task_pool = ComputeTaskPool::get();

    let thread_chunk_size = (sections_to_update.len() / compute_task_pool.thread_num()).max(1);

    let object_query = &object_query;
    let children_query = &children_query;

    compute_task_pool.scope(|scope| {
        for thread_chunk in sections_to_update.chunks(thread_chunk_size) {
            scope.spawn(async move {
                for section in thread_chunk {
                    let mut iter = unsafe { object_query.iter_many_unsafe(section) };
                    while let Some((transform, mut global_transform, hitbox, children)) =
                        iter.fetch_next()
                    {
                        // TODO: This will only work for one hour until overflow messes it up
                        if transform.last_changed().get() < global_transform.last_changed().get() {
                            continue;
                        }

                        *global_transform = GlobalTransform2d::from(*transform);

                        if let Some((hitbox, mut global_hitbox)) = hitbox {
                            *global_hitbox = GlobalHitbox::from((
                                hitbox,
                                &global_transform.affine,
                                transform.angle,
                                transform.scale,
                            ));
                        }

                        let Some(children) = children else {
                            continue;
                        };

                        unsafe {
                            propagate_transform_recursive(
                                children,
                                children_query,
                                &global_transform,
                            );
                        }
                    }
                }
            });
        }
    });
}

unsafe fn propagate_transform_recursive<'w, 's, D: QueryData, F: QueryFilter>(
    children: &Children,
    children_query: &'w Query<'w, 's, D, F>,
    parent_transform: &GlobalTransform2d,
) where
    D: QueryData<
        Item<'w> = (
            &'w Transform2d,
            Mut<'w, GlobalTransform2d>,
            Option<&'w Children>,
        ),
    >,
{
    for child in children {
        let Ok((transform, mut global_transform, children)) = children_query.get_unchecked(*child)
        else {
            continue;
        };

        *global_transform = parent_transform.mul_transform(*transform);

        let Some(children) = children else {
            continue;
        };

        propagate_transform_recursive(children, children_query, &global_transform)
    }
}
