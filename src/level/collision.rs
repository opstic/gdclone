use std::f32::consts::{FRAC_1_PI, FRAC_2_PI};

use bevy::math::{Vec2, Vec2Swizzles, Vec3, Vec4, Vec4Swizzles};
use bevy::prelude::{
    Color, Component, Entity, GizmoConfigGroup, GizmoPrimitive2d, Gizmos, Primitive2d, Query, Res,
};

use crate::level::section::{GlobalSections, Section};
use crate::level::transform::{GlobalTransform2d, Transform2d};

#[derive(Component)]
pub(crate) enum Hitbox {
    Box {
        no_rotation: bool,
        offset: Option<Vec2>,
        half_extents: Vec2,
    },
    Slope {
        half_extents: Vec2,
    },
    Circle {
        radius: f32,
    },
}

impl Default for Hitbox {
    fn default() -> Self {
        Self::Box {
            no_rotation: false,
            offset: None,
            half_extents: Vec2::splat(15.),
        }
    }
}

#[derive(Component, Copy, Clone)]
pub(crate) struct GlobalHitbox {
    pub(crate) aabb: Vec4,
    specific: Option<GlobalHitboxKind>,
}

#[derive(Copy, Clone)]
pub(crate) enum GlobalHitboxKind {
    Obb { vertices: [Vec2; 4] },
    Triangle { vertices: [Vec2; 3] },
    Circle { center: Vec2, radius: f32 },
}

impl Primitive2d for GlobalHitbox {}

impl Default for GlobalHitbox {
    fn default() -> Self {
        Self {
            aabb: Vec4::ZERO,
            specific: None,
        }
    }
}

impl From<(&Hitbox, &Transform2d, &GlobalTransform2d)> for GlobalHitbox {
    fn from(
        (hitbox, transform, global_transform): (&Hitbox, &Transform2d, &GlobalTransform2d),
    ) -> Self {
        let affine = global_transform.affine();
        match *hitbox {
            Hitbox::Box {
                no_rotation,
                offset,
                half_extents,
            } => {
                let transformed_center = if let Some(offset) = offset {
                    offset * transform.scale + affine.translation
                } else {
                    affine.translation
                };

                if no_rotation || (transform.angle * FRAC_2_PI).fract() == 0. {
                    let mut transformed_half_extents = (half_extents * transform.scale).abs();

                    if (transform.angle * FRAC_1_PI).fract() != 0. {
                        transformed_half_extents = transformed_half_extents.yx()
                    }

                    Self {
                        aabb: Vec4::from((
                            transformed_center - transformed_half_extents,
                            -(transformed_center + transformed_half_extents),
                        )),
                        specific: None,
                    }
                } else {
                    let mut vertices = [
                        Vec2::new(-half_extents.x, half_extents.y),
                        half_extents,
                        Vec2::new(half_extents.x, -half_extents.y),
                        -half_extents,
                    ];

                    for vertex in &mut vertices {
                        *vertex = affine.transform_point2(*vertex);
                    }

                    let x = Vec4::new(vertices[0].x, vertices[1].x, vertices[2].x, vertices[3].x);
                    let y = Vec4::new(vertices[0].y, vertices[1].y, vertices[2].y, vertices[3].y);

                    let min = Vec2::new(x.min_element(), y.min_element());
                    let max = Vec2::new(x.max_element(), y.max_element());

                    Self {
                        aabb: Vec4::from((min, -max)),
                        specific: Some(GlobalHitboxKind::Obb { vertices }),
                    }
                }
            }
            Hitbox::Slope { half_extents } => {
                let mut vertices = [
                    half_extents,
                    Vec2::new(half_extents.x, -half_extents.y),
                    -half_extents,
                ];

                for vertex in &mut vertices {
                    *vertex = affine.transform_point2(*vertex);
                }

                let mut transformed_half_extents = half_extents * transform.scale;

                if (transform.angle * FRAC_1_PI).fract() != 0. {
                    transformed_half_extents = transformed_half_extents.yx()
                }

                Self {
                    aabb: Vec4::from((
                        affine.translation - transformed_half_extents,
                        -(affine.translation + transformed_half_extents),
                    )),
                    specific: Some(GlobalHitboxKind::Triangle { vertices }),
                }
            }
            Hitbox::Circle { radius } => {
                let scaled_radius = (radius * transform.scale.max_element()).abs();
                let half_extents = Vec2::splat(scaled_radius);
                let min_point = affine.translation - half_extents;
                let max_point = affine.translation + half_extents;

                Self {
                    aabb: Vec4::from((min_point, -max_point)),
                    specific: Some(GlobalHitboxKind::Circle {
                        center: affine.translation,
                        radius: scaled_radius,
                    }),
                }
            }
        }
    }
}

impl GlobalHitbox {
    #[inline]
    pub(crate) fn intersect(&self, other: &GlobalHitbox) -> (bool, Option<Vec2>) {
        if self.aabb.cmplt(-other.aabb.zwxy()).any() {
            return (false, None);
        }

        (true, None)
    }
}

impl<'w, 's, T: GizmoConfigGroup> GizmoPrimitive2d<GlobalHitbox> for Gizmos<'w, 's, T> {
    type Output<'a> = () where Self: 'a;

    fn primitive_2d(
        &mut self,
        primitive: GlobalHitbox,
        _: Vec2,
        _: f32,
        color: Color,
    ) -> Self::Output<'_> {
        let scale = -primitive.aabb.zw() - primitive.aabb.xy();

        let Some(specific) = primitive.specific else {
            self.rect_2d(primitive.aabb.xy() + scale / 2., 0., scale, color);
            return;
        };

        self.rect_2d(
            primitive.aabb.xy() + scale / 2.,
            0.,
            scale,
            Color::rgb_from_array(Vec3::ONE - color.rgb_to_vec3()).with_a(0.1),
        );

        match specific {
            GlobalHitboxKind::Obb { vertices } => self.linestrip_2d(
                vertices
                    .into_iter()
                    .chain(std::iter::once(*vertices.first().unwrap())),
                color,
            ),
            GlobalHitboxKind::Triangle { vertices } => {
                self.linestrip_2d(
                    vertices
                        .into_iter()
                        .chain(std::iter::once(*vertices.first().unwrap())),
                    color,
                );
            }
            GlobalHitboxKind::Circle { center, radius } => {
                drop(self.circle_2d(center, radius, color))
            }
        }
    }
}

#[derive(Component, Default)]
pub(crate) struct ActiveCollider {
    pub(crate) collided: Vec<(Entity, GlobalHitbox, Option<Vec2>, bool)>,
}

pub(crate) fn update_collision(
    sections: Res<GlobalSections>,
    mut active_colliders: Query<(Entity, &mut ActiveCollider, &GlobalHitbox, &Section)>,
    others: Query<(Entity, &GlobalHitbox)>,
) {
    for (collider_entity, mut active_collider, collider_hitbox, collider_section) in
        &mut active_colliders
    {
        active_collider.collided.clear();

        for (other_entity, other_hitbox) in others.iter_many(
            sections.sections[collider_section.current as usize]
                .iter()
                .filter(|entity| **entity != collider_entity),
        ) {
            let (collided, collided_vector) = collider_hitbox.intersect(other_hitbox);
            if collided {
                active_collider
                    .collided
                    .push((other_entity, *other_hitbox, collided_vector, true));
            }
        }
    }
}
