use bevy::math::{Affine2, Mat2, Vec2, Vec2Swizzles, Vec4, Vec4Swizzles};
use bevy::prelude::{Color, Component, GizmoConfigGroup, GizmoPrimitive2d, Gizmos, Primitive2d};

#[derive(Component)]
pub(crate) enum Hitbox {
    Box {
        no_rotation: bool,
        offset: Vec2,
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
pub(crate) enum GlobalHitbox {
    Aabb(Vec4),
    Obb {
        center: Vec2,
        half_extents: Vec2,
        matrix: Mat2,
    },
    Triangle {
        min_max: Vec4,
        corner: Vec2,
    },
    Circle {
        center: Vec2,
        radius: f32,
    },
}

impl Primitive2d for GlobalHitbox {}

impl Default for GlobalHitbox {
    fn default() -> Self {
        Self::Aabb(Vec4::ZERO)
    }
}

impl GlobalHitbox {
    pub(crate) fn calculate(
        hitbox: &Hitbox,
        affine: Affine2,
        scale: Vec2,
        is_axis_aligned: bool,
    ) -> Self {
        match *hitbox {
            Hitbox::Box {
                no_rotation,
                offset: center,
                half_extents,
            } => {
                if no_rotation || is_axis_aligned {
                    let min_point = affine.transform_point2(center - half_extents);
                    let max_point = affine.transform_point2(center + half_extents);
                    Self::Aabb(Vec4::from((min_point, -max_point)))
                } else {
                    Self::Obb {
                        center: affine.transform_point2(center),
                        half_extents: half_extents * scale,
                        matrix: affine.matrix2,
                    }
                }
            }
            Hitbox::Slope { half_extents } => {
                let min_point = affine.transform_point2(-half_extents);
                let max_point = affine.transform_point2(half_extents);
                let corner = affine.transform_point2(Vec2::new(half_extents.x, -half_extents.y));
                Self::Triangle {
                    min_max: Vec4::from((min_point, -max_point)),
                    corner,
                }
            }
            Hitbox::Circle { radius } => Self::Circle {
                center: affine.translation,
                radius: radius * scale.x,
            },
        }
    }

    #[inline]
    pub(crate) fn intersect(&self, other: &GlobalHitbox) -> bool {
        let Self::Aabb(min_max) = self else { todo!() };

        match other {
            GlobalHitbox::Aabb(other) => min_max.cmplt(-other.zwxy()).bitmask() == 0,
            GlobalHitbox::Obb { .. } => {
                todo!()
            }
            _ => todo!(),
        }
    }

    #[inline]
    pub(crate) fn translation_rotation_scale(&self) -> (Vec2, f32, Vec2) {
        match self {
            GlobalHitbox::Aabb(min_max) => {
                let scale = -min_max.zw() - min_max.xy();

                (min_max.xy() + scale / 2., 0., scale)
            }
            GlobalHitbox::Obb {
                center,
                half_extents,
                matrix,
            } => (*center, -matrix.y_axis.yx().to_angle(), *half_extents * 2.),
            _ => todo!(),
        }
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
        match primitive {
            GlobalHitbox::Aabb(min_max) => {
                let scale = -min_max.zw() - min_max.xy();

                self.rect_2d(min_max.xy() + scale / 2., 0., scale, color)
            }
            GlobalHitbox::Obb {
                center,
                half_extents,
                matrix,
            } => self.rect_2d(
                center,
                -matrix.y_axis.yx().to_angle(),
                half_extents * 2.,
                color,
            ),
            GlobalHitbox::Triangle { min_max, corner } => {
                let [p1, p2, p3] = [min_max.xy(), -min_max.zw(), corner];
                self.linestrip_2d([p1, p2, p3, p1], color);
            }
            GlobalHitbox::Circle { center, radius } => drop(self.circle_2d(center, radius, color)),
        }
    }
}
