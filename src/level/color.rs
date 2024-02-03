use std::sync::Mutex;

use bevy::ecs::{
    component::Tick,
    query::{ReadOnlyWorldQuery, WorldQuery},
};
use bevy::hierarchy::{BuildChildren, BuildWorldChildren, Children, Parent};
use bevy::prelude::{
    Color, Commands, Component, DetectChanges, DetectChangesMut, Entity, Mut, Query, Ref, Res,
    ResMut, Resource, With, Without, World,
};
use bevy::reflect::Reflect;
use bevy::tasks::ComputeTaskPool;
use bevy::utils::{hashbrown, HashMap as AHashMap};
use dashmap::DashMap;
use serde::Deserialize;

use crate::level::group::{GroupArchetypeCalculated, ObjectGroups};
use crate::level::{de, section::GlobalSections};
use crate::utils::{hsv_to_rgb, rgb_to_hsv, u8_to_bool, U64Hash};

#[derive(Default, Resource)]
pub(crate) struct GlobalColorChannels(pub(crate) DashMap<u64, Entity, U64Hash>);

#[derive(Component, Debug)]
pub(crate) enum GlobalColorChannel {
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

impl GlobalColorChannel {
    pub(crate) fn parse(color_string: &[u8]) -> Result<(u64, GlobalColorChannel), anyhow::Error> {
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
            GlobalColorChannel::Copy {
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
            GlobalColorChannel::Base {
                color: temp_color,
                blending,
            }
        };
        Ok((index, color))
    }
}

impl Default for GlobalColorChannel {
    fn default() -> Self {
        GlobalColorChannel::Base {
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
    let mut query = world.query::<&GlobalColorChannel>();
    for entry_ref in global_color_channels.0.iter() {
        let color_channel_entity = *entry_ref.value();
        let Ok(color_channel) = query.get(world, color_channel_entity) else {
            continue;
        };
        match color_channel {
            GlobalColorChannel::Copy { copied_index, .. } => {
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
            GlobalColorChannel::Base { .. } => continue,
        }
    }
    for (index, dependent_entities) in channels_to_add {
        let mut blank_color_channel_entity = world.spawn((
            GlobalColorChannel::default(),
            ColorChannelCalculated::default(),
        ));
        for entity in dependent_entities {
            blank_color_channel_entity.add_child(entity);
        }
        global_color_channels
            .0
            .insert(index, blank_color_channel_entity.id());
    }
}

#[derive(Default, Component)]
pub(crate) struct ColorChannelCalculated {
    pub(crate) color: Color,
    pub(crate) blending: bool,
    deferred: bool,
}

pub(crate) fn update_color_channel_calculated(
    commands: Commands,
    mut global_color_channels: ResMut<GlobalColorChannels>,
    mut root_color_channels: Query<
        (
            Entity,
            Ref<GlobalColorChannel>,
            &mut ColorChannelCalculated,
            Option<&Children>,
        ),
        Without<Parent>,
    >,
    child_color_channels: Query<
        (
            Entity,
            Ref<GlobalColorChannel>,
            &mut ColorChannelCalculated,
            Option<&Children>,
        ),
        With<Parent>,
    >,
) {
    let mutex = Mutex::new((commands, &mut *global_color_channels));

    root_color_channels.par_iter_mut().for_each(
        |(entity, color_channel, mut calculated, children)| {
            let should_update = color_channel.is_changed() || calculated.deferred;

            let (color, blending) = match *color_channel {
                GlobalColorChannel::Base { color, blending } => (color, blending),
                GlobalColorChannel::Copy { copied_index, .. } => {
                    // Fix the hierarchy for the next iteration
                    let (commands, global_color_channels) = &mut *mutex.lock().unwrap();
                    let mut parent_entity =
                        if let Some(parent_entity) = global_color_channels.0.get(&copied_index) {
                            if *parent_entity == entity {
                                // Recursive color channel
                                calculated.deferred = true;
                                return;
                            }
                            commands.entity(*parent_entity)
                        } else {
                            let entity = commands.spawn((
                                GlobalColorChannel::default(),
                                ColorChannelCalculated::default(),
                            ));
                            global_color_channels.0.insert(copied_index, entity.id());
                            entity
                        };

                    parent_entity.add_child(entity);

                    calculated.deferred = true;
                    return;
                }
            };

            if should_update {
                calculated.color = color;
                calculated.blending = blending;
                calculated.deferred = false;
            }

            let Some(children) = children else {
                return;
            };

            unsafe {
                recursive_propagate_color(
                    &mutex,
                    children,
                    color,
                    &child_color_channels,
                    should_update,
                )
            }
        },
    );
}

unsafe fn recursive_propagate_color<'w, 's, Q: WorldQuery, F: ReadOnlyWorldQuery>(
    mutex: &Mutex<(Commands, &mut GlobalColorChannels)>,
    children: &Children,
    parent_color: Color,
    children_query: &'w Query<'w, 's, Q, F>,
    should_update: bool,
) where
    Q: WorldQuery<
        Item<'w> = (
            Entity,
            Ref<'w, GlobalColorChannel>,
            Mut<'w, ColorChannelCalculated>,
            Option<&'w Children>,
        ),
    >,
{
    for child in children {
        let Ok((entity, color_channel, mut calculated, children)) =
            children_query.get_unchecked(*child)
        else {
            continue;
        };

        let should_update = should_update || color_channel.is_changed() || calculated.deferred;

        let GlobalColorChannel::Copy {
            copy_opacity,
            opacity,
            blending,
            hsv,
            ..
        } = *color_channel
        else {
            // Fix the hierarchy for the next iteration
            let (commands, _) = &mut *mutex.lock().unwrap();

            commands.entity(entity).remove_parent();
            calculated.deferred = true;
            continue;
        };

        if should_update {
            let mut temp_color = parent_color;
            if !copy_opacity {
                temp_color.set_a(opacity);
            }

            if let Some(hsv) = hsv {
                hsv.apply_rgb(&mut temp_color);
            }

            calculated.color = temp_color;
            calculated.blending = blending;
            calculated.deferred = false;
        }

        let Some(children) = children else {
            continue;
        };

        unsafe {
            recursive_propagate_color(
                mutex,
                children,
                calculated.color,
                children_query,
                should_update,
            )
        }
    }
}

#[derive(Component)]
pub(crate) struct ObjectColor {
    pub(crate) channel_id: u64,
    pub(crate) channel_entity: Entity,
    pub(crate) hsv: Option<HsvMod>,
    pub(crate) object_opacity: f32,
    pub(crate) object_color_kind: ObjectColorKind,
}

impl Default for ObjectColor {
    fn default() -> Self {
        Self {
            channel_id: u64::MAX,
            channel_entity: Entity::PLACEHOLDER,
            hsv: None,
            object_opacity: 1.,
            object_color_kind: ObjectColorKind::None,
        }
    }
}

#[derive(Component)]
pub(crate) struct ObjectColorCalculated {
    pub(crate) color: Color,
    pub(crate) blending: bool,
    pub(crate) enabled: bool,
}

impl Default for ObjectColorCalculated {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            blending: false,
            enabled: true,
        }
    }
}

pub(crate) fn update_object_color(
    global_sections: Res<GlobalSections>,
    global_color_channels: Res<GlobalColorChannels>,
    group_archetypes: Query<Ref<GroupArchetypeCalculated>>,
    objects: Query<(&ObjectGroups, &mut ObjectColor, &mut ObjectColorCalculated)>,
    color_channels: Query<Ref<ColorChannelCalculated>>,
) {
    let sections_to_update = &global_sections.sections[global_sections.visible.clone()];

    let compute_task_pool = ComputeTaskPool::get();

    let thread_chunk_size = (sections_to_update.len() / compute_task_pool.thread_num()).max(1);

    let objects = &objects;
    let color_channels = &color_channels;
    let global_color_channels = &global_color_channels;
    let group_archetypes = &group_archetypes;

    compute_task_pool.scope(|scope| {
        for thread_chunk in sections_to_update.chunks(thread_chunk_size) {
            scope.spawn(async move {
                for section in thread_chunk {
                    let mut iter = unsafe { objects.iter_many_unsafe(section) };
                    while let Some((object_groups, mut object_color, mut calculated)) =
                        iter.fetch_next()
                    {
                        let group_archetype = group_archetypes
                            .get(object_groups.archetype_entity)
                            .unwrap();

                        if !group_archetype.enabled {
                            calculated.bypass_change_detection().enabled = false;
                            continue;
                        }
                        calculated.bypass_change_detection().enabled = true;

                        let (mut color_channel_color, blending, color_channel_tick) =
                            if let Ok(color_channel_calculated) =
                                color_channels.get(object_color.channel_entity)
                            {
                                (
                                    color_channel_calculated.color,
                                    color_channel_calculated.blending,
                                    color_channel_calculated.last_changed(),
                                )
                            } else if let Some(entity) =
                                global_color_channels.0.get(&object_color.channel_id)
                            {
                                object_color.channel_entity = *entity;
                                if let Ok(color_channel_calculated) = color_channels.get(*entity) {
                                    (
                                        color_channel_calculated.color,
                                        color_channel_calculated.blending,
                                        color_channel_calculated.last_changed(),
                                    )
                                } else {
                                    (Color::WHITE, false, Tick::new(0))
                                }
                            } else {
                                (Color::WHITE, false, Tick::new(0))
                            };

                        // TODO: This will only work for one hour until overflow messes it up
                        let most_recent_change = color_channel_tick
                            .get()
                            .max(group_archetype.last_changed().get());
                        if most_recent_change < calculated.last_changed().get() {
                            continue;
                        }

                        let alpha = group_archetype.opacity
                            * object_color.object_opacity
                            * color_channel_color.a();

                        let color = match object_color.object_color_kind {
                            ObjectColorKind::None => Color::WHITE.with_a(alpha),
                            ObjectColorKind::Black => Color::BLACK.with_a(alpha),
                            _ => {
                                if let Some(hsv) = object_color.hsv {
                                    hsv.apply_rgb(&mut color_channel_color);
                                }
                                color_channel_color.with_a(alpha)
                            }
                        };

                        calculated.color = color;
                        calculated.blending = blending;
                    }
                }
            });
        }
    });
}

#[derive(Default, Copy, Clone, Component, PartialEq)]
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
        let h: f32 = std::str::from_utf8(hsv_data[0])?.parse()?;
        let s = std::str::from_utf8(hsv_data[1])?.parse()?;
        let v = std::str::from_utf8(hsv_data[2])?.parse()?;
        let s_absolute = u8_to_bool(hsv_data[3]);
        let v_absolute = u8_to_bool(hsv_data[4]);

        Ok(HsvMod {
            h: h * (1. / 60.),
            s,
            v,
            s_absolute,
            v_absolute,
        })
    }
}

impl HsvMod {
    pub(crate) fn new(h: f32, s: f32, v: f32, s_absolute: bool, v_absolute: bool) -> Self {
        Self {
            h,
            s,
            v,
            s_absolute,
            v_absolute,
        }
    }

    pub(crate) fn apply_rgb(&self, color: &mut Color) {
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
        color.set_r(r);
        color.set_g(g);
        color.set_b(b);
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
