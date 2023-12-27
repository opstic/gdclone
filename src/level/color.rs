use bevy::ecs::query::{ReadOnlyWorldQuery, WorldQuery};
use bevy::hierarchy::{BuildWorldChildren, Children, Parent};
use bevy::log::warn;
use bevy::prelude::{
    Color, Component, DetectChanges, Entity, Mut, Query, Ref, Res, Resource, With, Without, World,
};
use bevy::reflect::Reflect;
use bevy::utils::{hashbrown, HashMap as AHashMap};
use bevy_enum_filter::EnumFilter;
use dashmap::DashMap;
use serde::Deserialize;

use crate::level::de;
use crate::level::section::{GlobalSections, SectionIndex, VisibleGlobalSections};
use crate::utils::{hsv_to_rgb, rgb_to_hsv, u8_to_bool, U64Hash};

#[derive(Default, Resource)]
pub(crate) struct GlobalColorChannels(pub(crate) DashMap<u64, Entity, U64Hash>);

#[derive(Component)]
pub(crate) enum ColorChannel {
    Base {
        color: Color,
        blending: bool,
    },
    Copy {
        copied_index: u64,
        copy_opacity: bool,
        opacity: f32,
        blending: bool,
        hsv: Option<HsvMod>,
    },
}

impl ColorChannel {
    pub(crate) fn parse(color_string: &[u8]) -> Result<(u64, ColorChannel), anyhow::Error> {
        let color_data: AHashMap<&[u8], &[u8]> = de::from_slice(color_string, b'_')?;
        let index = std::str::from_utf8(
            color_data
                .get(b"6".as_ref())
                .ok_or(anyhow::Error::msg("No index in color???"))?,
        )?
        .parse()?;
        let color = if color_data.contains_key(b"9".as_ref()) {
            let copied_index = if let Some(copied_index) = color_data.get(b"9".as_ref()) {
                std::str::from_utf8(copied_index)?.parse()?
            } else {
                u64::MAX
            };
            let copy_opacity = if let Some(copy_opacity) = color_data.get(b"17".as_ref()) {
                u8_to_bool(copy_opacity)
            } else {
                false
            };
            let opacity = if let Some(opacity) = color_data.get(b"7".as_ref()) {
                std::str::from_utf8(opacity)?.parse()?
            } else {
                1.
            };
            let blending = if let Some(blending) = color_data.get(b"5".as_ref()) {
                u8_to_bool(blending)
            } else {
                false
            };
            let hsv = if let Some(hsv) = color_data.get(b"10".as_ref()) {
                Some(HsvMod::parse(hsv)?)
            } else {
                None
            };
            ColorChannel::Copy {
                copied_index,
                copy_opacity,
                opacity,
                blending,
                hsv,
            }
        } else {
            let mut temp_color = Color::WHITE;
            if let Some(r) = color_data.get(b"1".as_ref()) {
                temp_color.set_r(std::str::from_utf8(r)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(g) = color_data.get(b"2".as_ref()) {
                temp_color.set_g(std::str::from_utf8(g)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(b) = color_data.get(b"3".as_ref()) {
                temp_color.set_b(std::str::from_utf8(b)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(opacity) = color_data.get(b"7".as_ref()) {
                temp_color.set_a(std::str::from_utf8(opacity)?.parse()?);
            }
            let blending = if let Some(blending) = color_data.get(b"5".as_ref()) {
                u8_to_bool(blending)
            } else {
                false
            };
            ColorChannel::Base {
                color: temp_color,
                blending,
            }
        };
        Ok((index, color))
    }
}

impl Default for ColorChannel {
    fn default() -> Self {
        ColorChannel::Base {
            color: Color::WHITE,
            blending: false,
        }
    }
}

pub(crate) fn construct_color_channel_hierarchy(
    world: &mut World,
    global_color_channels: &mut GlobalColorChannels,
) {
    let mut channels_to_add: hashbrown::HashMap<u64, Vec<Entity>, U64Hash> =
        hashbrown::HashMap::with_hasher(U64Hash);
    let mut query = world.query::<&ColorChannel>();
    for entry_ref in global_color_channels.0.iter() {
        let color_channel_entity = *entry_ref.value();
        let Ok(color_channel) = query.get(world, color_channel_entity) else {
            continue;
        };
        match color_channel {
            ColorChannel::Copy { copied_index, .. } => {
                let copying_entity = if let Some(entity) = global_color_channels.0.get(copied_index)
                {
                    *entity
                } else {
                    // Delegate it to later
                    let channel_to_add = channels_to_add.entry(*copied_index).or_default();
                    channel_to_add.push(color_channel_entity);
                    continue;
                };

                world
                    .entity_mut(copying_entity)
                    .add_child(color_channel_entity);
            }
            ColorChannel::Base { .. } => continue,
        }
    }
    for (index, dependent_entities) in channels_to_add {
        let mut blank_color_channel_entity =
            world.spawn((ColorChannel::default(), ColorChannelCalculated::default()));
        for entity in dependent_entities {
            blank_color_channel_entity.add_child(entity);
        }
        global_color_channels
            .0
            .insert(index, blank_color_channel_entity.id());
    }
}

#[derive(Default, Component)]
pub(crate) struct ColorChannelCalculated(Color, bool);

pub(crate) fn update_color_channel_calculated(
    mut root_color_channels: Query<
        (
            Ref<ColorChannel>,
            &mut ColorChannelCalculated,
            Option<&Children>,
        ),
        Without<Parent>,
    >,
    child_color_channels: Query<
        (
            Ref<ColorChannel>,
            &mut ColorChannelCalculated,
            Option<&Children>,
        ),
        With<Parent>,
    >,
) {
    root_color_channels.par_iter_mut().for_each(
        |(color_channel, mut color_channel_calculated, children)| {
            let should_update = color_channel.is_changed();

            let ColorChannel::Base { color, blending } = *color_channel else {
                warn!("Root color channel is a copy channel???");
                return;
            };

            if should_update {
                color_channel_calculated.0 = color;
                color_channel_calculated.1 = blending;

                if blending {
                    let squared_alpha = color_channel_calculated.0.a().powf(2.);
                    color_channel_calculated.0.set_a(squared_alpha);
                }
            }

            let Some(children) = children else {
                return;
            };

            unsafe {
                recursive_propagate_color(children, color, &child_color_channels, should_update)
            }
        },
    );
}

unsafe fn recursive_propagate_color<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery>(
    children: &Children,
    parent_color: Color,
    children_query: &'w Query<'w, 's, Q, F>,
    should_update: bool,
) where
    Q: WorldQuery<
        Item<'w> = (
            Ref<'w, ColorChannel>,
            Mut<'w, ColorChannelCalculated>,
            Option<&'w Children>,
        ),
    >,
{
    for child in children {
        let Ok((color_channel, mut calculated, children)) = children_query.get_unchecked(*child)
        else {
            continue;
        };

        let should_update = should_update || color_channel.is_changed();

        let ColorChannel::Copy {
            copy_opacity,
            opacity,
            blending,
            hsv,
            ..
        } = *color_channel
        else {
            warn!("Child color channel is a base channel???");
            continue;
        };

        if should_update {
            let mut temp_color = parent_color;
            if !copy_opacity {
                temp_color.set_a(opacity);
            }

            if let Some(hsv) = hsv {
                temp_color = hsv.apply(temp_color);
            }

            calculated.0 = temp_color;
            calculated.1 = blending;

            if blending {
                let squared_alpha = calculated.0.a().powf(2.);
                calculated.0.set_a(squared_alpha);
            }
        }

        let Some(children) = children else {
            return;
        };

        unsafe { recursive_propagate_color(children, calculated.0, children_query, should_update) }
    }
}

#[derive(Component)]
pub(crate) struct ObjectColor {
    pub(crate) channel_id: u64,
    pub(crate) channel_entity: Entity,
    pub(crate) hsv: Option<HsvMod>,
    pub(crate) object_opacity: f32,
    pub(crate) object_color_kind: ObjectColorKind,
    pub(crate) color: Color,
    pub(crate) blending: bool,
}

impl Default for ObjectColor {
    fn default() -> Self {
        Self {
            channel_id: u64::MAX,
            channel_entity: Entity::PLACEHOLDER,
            hsv: None,
            object_opacity: 1.,
            object_color_kind: ObjectColorKind::None,
            color: Color::WHITE,
            blending: false,
        }
    }
}

pub(crate) fn update_object_color(
    global_sections: Res<GlobalSections>,
    visible_global_sections: Res<VisibleGlobalSections>,
    global_color_channels: Res<GlobalColorChannels>,
    mut objects: Query<&mut ObjectColor>,
    color_channels: Query<Ref<ColorChannelCalculated>>,
) {
    for x in visible_global_sections.x.clone() {
        for y in visible_global_sections.y.clone() {
            let section_index = SectionIndex::new(x, y);
            let Some(global_section) = global_sections.0.get(&section_index) else {
                continue;
            };

            let mut iter = objects.iter_many_mut(&*global_section);

            while let Some(mut object_color) = iter.fetch_next() {
                let Ok(color_channel_calculated) = color_channels.get(object_color.channel_entity)
                else {
                    // TODO: Try to get the new channel
                    // if object_color.is_changed() {
                    //
                    // }
                    continue;
                };
                if !(color_channel_calculated.is_changed() || object_color.is_changed()) {
                    continue;
                }
                let mut color = if object_color.object_color_kind == ObjectColorKind::None {
                    Color::WHITE.with_a(color_channel_calculated.0.a())
                } else if object_color.object_color_kind == ObjectColorKind::Black {
                    Color::BLACK.with_a(color_channel_calculated.0.a())
                } else if let Some(hsv) = object_color.hsv {
                    hsv.apply(color_channel_calculated.0)
                } else {
                    color_channel_calculated.0
                };

                color.set_a(object_color.object_opacity * color.a());

                object_color.color = color;
                object_color.blending = color_channel_calculated.1;
            }
        }
    }
}

#[derive(Default, Copy, Clone, Component, EnumFilter, PartialEq)]
pub(crate) enum ObjectColorKind {
    Base,
    Detail,
    Black,
    #[default]
    None,
}

#[derive(Component, Debug, Deserialize, Copy, Clone, Reflect)]
pub(crate) struct HsvMod {
    pub(crate) h: f32,
    pub(crate) s: f32,
    pub(crate) v: f32,
    pub(crate) s_absolute: bool,
    pub(crate) v_absolute: bool,
}

impl HsvMod {
    pub(crate) fn parse(hsv_string: &[u8]) -> Result<HsvMod, anyhow::Error> {
        let hsv_data: [&[u8]; 5] = de::from_slice(hsv_string, b'a')?;
        Ok(HsvMod {
            h: std::str::from_utf8(hsv_data[0])?.parse()?,
            s: std::str::from_utf8(hsv_data[1])?.parse()?,
            v: std::str::from_utf8(hsv_data[2])?.parse()?,
            s_absolute: u8_to_bool(hsv_data[3]),
            v_absolute: u8_to_bool(hsv_data[4]),
        })
    }
}

impl HsvMod {
    pub(crate) fn apply(&self, color: Color) -> Color {
        let (h, s, v) = rgb_to_hsv([color.r(), color.g(), color.b()]);
        let [r, g, b] = hsv_to_rgb((
            h + self.h,
            if self.s_absolute {
                s + self.s
            } else {
                s * self.s
            },
            if self.v_absolute {
                v + self.v
            } else {
                v * self.v
            },
        ));
        Color::rgba(r, g, b, color.a())
    }
}

impl Default for HsvMod {
    fn default() -> Self {
        HsvMod {
            h: 0.,
            s: 1.,
            v: 1.,
            s_absolute: false,
            v_absolute: false,
        }
    }
}
