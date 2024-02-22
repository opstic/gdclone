use std::f32::consts::{FRAC_1_PI, FRAC_2_PI};

use bevy::math::{Affine2, Vec2, Vec2Swizzles, Vec4, Vec4Swizzles};
use bevy::prelude::{Color, Component, GizmoConfigGroup, GizmoPrimitive2d, Gizmos, Primitive2d};

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

#[derive(Component, Copy, Clone)]
pub(crate) struct GlobalHitbox {
    aabb: Vec4,
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

impl From<(&Hitbox, &Affine2, f32, Vec2)> for GlobalHitbox {
    fn from((hitbox, affine, angle, scale): (&Hitbox, &Affine2, f32, Vec2)) -> Self {
        match *hitbox {
            Hitbox::Box {
                no_rotation,
                offset,
                half_extents,
            } => {
                let transformed_center = if let Some(offset) = offset {
                    offset * scale + affine.translation
                } else {
                    affine.translation
                };

                if no_rotation || (angle * FRAC_2_PI).fract() == 0. {
                    let mut transformed_half_extents = (half_extents * scale).abs();

                    if (angle * FRAC_1_PI).fract() != 0. {
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

                let mut transformed_half_extents = half_extents * scale;

                if (angle * FRAC_1_PI).fract() != 0. {
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
                let scaled_radius = (radius * scale.max_element()).abs();
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
            color.with_a(0.25),
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
