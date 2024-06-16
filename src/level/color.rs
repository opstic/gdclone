use std::hash::Hash;

use bevy::asset::{AssetId, Handle};
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::hierarchy::{BuildChildren, BuildWorldChildren, Children, Parent};
use bevy::math::{Vec3, Vec3A, Vec4};
use bevy::prelude::{
    Component, DetectChanges, DetectChangesMut, Entity, Image, Mut, ParallelCommands, Query, Ref,
    Res, Resource, With, Without, World,
};
use bevy::reflect::Reflect;
use bevy::tasks::ComputeTaskPool;
use bevy::utils::{default, hashbrown, HashMap as AHashMap};
use dashmap::DashMap;
use serde::Deserialize;

use crate::level::group::{GroupArchetypeCalculated, ObjectGroups};
use crate::level::{de, section::GlobalSections};
use crate::utils::{hsv_to_rgb, rgb_to_hsv, str_to_bool, U64Hash};

#[derive(Default, Resource)]
pub(crate) struct GlobalColorChannels(pub(crate) DashMap<u64, Entity, U64Hash>);

#[derive(Component, Debug, Default)]
pub(crate) struct GlobalColorChannel {
    pub(crate) id: u64,
    pub(crate) kind: GlobalColorChannelKind,
}

#[derive(Debug)]
pub(crate) enum GlobalColorChannelKind {
    Base {
        color: Vec4,
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
    pub(crate) fn parse(color_string: &str) -> Result<GlobalColorChannel, anyhow::Error> {
        let color_data: AHashMap<&str, &str> = de::from_str(color_string, '_')?;
        let index = color_data
            .get("6")
            .ok_or(anyhow::Error::msg("No index in color???"))?
            .parse()?;
        let player = if let Some(copied_player) = color_data.get("4") {
            copied_player.parse()?
        } else {
            -1
        };
        let color = if color_data.contains_key("9") || player != -1 {
            let copied_index = if let Some(copied_index) = color_data.get("9") {
                copied_index.parse()?
            } else if player != -1 {
                match player {
                    1 => 1005,
                    2 => 1006,
                    _ => unreachable!(),
                }
            } else {
                u64::MAX
            };
            let copy_opacity = if let Some(copy_opacity) = color_data.get("17") {
                str_to_bool(copy_opacity)
            } else {
                false
            };
            let opacity = if let Some(opacity) = color_data.get("7") {
                opacity.parse()?
            } else {
                1.
            };
            let blending = if let Some(blending) = color_data.get("5") {
                str_to_bool(blending)
            } else {
                false
            };
            let hsv = if let Some(hsv) = color_data.get("10") {
                Some(HsvMod::parse(hsv)?)
            } else {
                None
            };
            GlobalColorChannelKind::Copy {
                copied_index,
                copy_opacity,
                opacity,
                blending,
                hsv,
            }
        } else {
            let mut temp_color = Vec4::ONE;
            if let Some(r) = color_data.get("1") {
                temp_color[0] = r.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(g) = color_data.get("2") {
                temp_color[1] = g.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(b) = color_data.get("3") {
                temp_color[2] = b.parse::<u8>()? as f32 / u8::MAX as f32;
            }
            if let Some(opacity) = color_data.get("7") {
                temp_color[3] = opacity.parse()?;
            }
            let blending = if let Some(blending) = color_data.get("5") {
                str_to_bool(blending)
            } else {
                false
            };
            GlobalColorChannelKind::Base {
                color: temp_color,
                blending,
            }
        };
        Ok(Self {
            id: index,
            kind: color,
        })
    }
}

impl Default for GlobalColorChannelKind {
    fn default() -> Self {
        GlobalColorChannelKind::Base {
            color: Vec4::ONE,
            blending: false,
        }
    }
}

#[derive(Component, Default)]
pub(crate) struct Pulses {
    pub(crate) pulses: Vec<(f32, ColorMod, ObjectColorKind)>,
}

pub(crate) fn clear_pulses(mut pulses: Query<&mut Pulses>) {
    for mut pulses in &mut pulses {
        if !pulses.pulses.is_empty() {
            pulses.pulses.clear();
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
        match color_channel.kind {
            GlobalColorChannelKind::Copy { copied_index, .. } => {
                let copying_entity =
                    if let Some(entity) = global_color_channels.0.get(&copied_index) {
                        *entity
                    } else {
                        // Delegate it to later
                        let channel_to_add = channels_to_add.entry(copied_index).or_default();
                        channel_to_add.push(color_channel_entity);
                        continue;
                    };

                world
                    .entity_mut(copying_entity)
                    .add_child(color_channel_entity);
            }
            GlobalColorChannelKind::Base { .. } => continue,
        }
    }
    for (index, dependent_entities) in channels_to_add {
        let mut blank_color_channel_entity = world.spawn((
            GlobalColorChannel::default(),
            ColorChannelCalculated::default(),
            Pulses::default(),
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
    pub(crate) color: Vec4,
    pub(crate) pre_pulse_color: Vec4,
    pub(crate) blending: bool,
    deferred: bool,
}

pub(crate) fn update_color_channel_calculated(
    par_commands: ParallelCommands,
    global_color_channels: Res<GlobalColorChannels>,
    mut root_color_channels: Query<
        (
            Entity,
            Ref<GlobalColorChannel>,
            Ref<Pulses>,
            &mut ColorChannelCalculated,
            Option<&Children>,
        ),
        Without<Parent>,
    >,
    child_color_channels: Query<
        (
            Entity,
            Ref<GlobalColorChannel>,
            Ref<Pulses>,
            &mut ColorChannelCalculated,
            Option<&Children>,
        ),
        With<Parent>,
    >,
) {
    root_color_channels.par_iter_mut().for_each(
        |(entity, color_channel, pulses, mut calculated, children)| {
            let should_update =
                color_channel.is_changed() || calculated.deferred || pulses.is_changed();

            match color_channel.kind {
                GlobalColorChannelKind::Base { color, blending } => {
                    if should_update {
                        calculated.pre_pulse_color = color;
                        calculated.color = color;

                        ColorMod::apply_color_mods(
                            pulses
                                .pulses
                                .iter()
                                .map(|(progress, color_mod, _)| (*progress, *color_mod)),
                            &mut calculated.color,
                        );

                        calculated.blending = blending;
                        calculated.deferred = false;
                    }
                }
                GlobalColorChannelKind::Copy {
                    copied_index,
                    opacity,
                    blending,
                    hsv,
                    ..
                } => {
                    // Replicate GD behavior
                    if copied_index == color_channel.id {
                        calculated.pre_pulse_color = calculated.color;

                        calculated.pre_pulse_color[3] = opacity;

                        if let Some(hsv) = hsv {
                            hsv.apply_rgba(&mut calculated.pre_pulse_color);
                        }

                        calculated.color = calculated.pre_pulse_color;

                        ColorMod::apply_color_mods(
                            pulses
                                .pulses
                                .iter()
                                .map(|(progress, color_mod, _)| (*progress, *color_mod)),
                            &mut calculated.color,
                        );

                        calculated.blending = blending;
                        calculated.deferred = false;
                    } else {
                        // Fix the hierarchy for the next iteration
                        par_commands.command_scope(|mut commands| {
                            let mut parent_entity = if let Some(parent_entity) =
                                global_color_channels.0.get(&copied_index)
                            {
                                if *parent_entity == entity {
                                    // Recursive color channel
                                    calculated.deferred = true;
                                    return;
                                }
                                commands.entity(*parent_entity)
                            } else {
                                // Use a placeholder
                                commands.entity(*global_color_channels.0.get(&0).unwrap())
                            };

                            parent_entity.add_child(entity);
                        });

                        calculated.deferred = true;
                        return;
                    }
                }
            };

            let Some(children) = children else {
                return;
            };

            unsafe {
                recursive_propagate_color(
                    &par_commands,
                    children,
                    color_channel.id,
                    calculated.color,
                    &child_color_channels,
                    should_update,
                )
            }
        },
    );
}

unsafe fn recursive_propagate_color<'w, 's, D: QueryData, F: QueryFilter>(
    par_commands: &ParallelCommands,
    children: &Children,
    parent_id: u64,
    parent_color: Vec4,
    children_query: &'w Query<'w, 's, D, F>,
    should_update: bool,
) where
    D: QueryData<
        Item<'w> = (
            Entity,
            Ref<'w, GlobalColorChannel>,
            Ref<'w, Pulses>,
            Mut<'w, ColorChannelCalculated>,
            Option<&'w Children>,
        ),
    >,
{
    for child in children {
        let Ok((entity, color_channel, pulses, mut calculated, children)) =
            children_query.get_unchecked(*child)
        else {
            continue;
        };

        let should_update = should_update
            || color_channel.is_changed()
            || calculated.deferred
            || pulses.is_changed();

        let GlobalColorChannelKind::Copy {
            copied_index,
            copy_opacity,
            opacity,
            blending,
            hsv,
            ..
        } = color_channel.kind
        else {
            // Fix the hierarchy for the next iteration
            par_commands.command_scope(|mut commands| {
                commands.entity(entity).remove_parent();
            });
            calculated.deferred = true;
            continue;
        };

        if parent_id != copied_index {
            // Fix the hierarchy for the next iteration
            par_commands.command_scope(|mut commands| {
                commands.entity(entity).remove_parent();
            });
            calculated.deferred = true;
            continue;
        }

        if should_update {
            calculated.pre_pulse_color = parent_color;

            if !copy_opacity {
                calculated.pre_pulse_color[3] = opacity;
            }

            if let Some(hsv) = hsv {
                hsv.apply_rgba(&mut calculated.pre_pulse_color);
            }

            calculated.color = calculated.pre_pulse_color;

            ColorMod::apply_color_mods(
                pulses
                    .pulses
                    .iter()
                    .map(|(progress, color_mod, _)| (*progress, *color_mod)),
                &mut calculated.color,
            );

            calculated.blending = blending;
            calculated.deferred = false;
        }

        let Some(children) = children else {
            continue;
        };

        unsafe {
            recursive_propagate_color(
                par_commands,
                children,
                color_channel.id,
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
    pub(crate) texture_ids: (AssetId<Image>, AssetId<Image>),
}

impl Default for ObjectColor {
    fn default() -> Self {
        Self {
            channel_id: u64::MAX,
            channel_entity: Entity::PLACEHOLDER,
            hsv: None,
            object_opacity: 1.,
            object_color_kind: ObjectColorKind::None,
            texture_ids: (AssetId::invalid(), AssetId::invalid()),
        }
    }
}

#[derive(Clone, Component, Copy)]
pub(crate) struct ObjectColorCalculated {
    pub(crate) color: Vec4,
    pub(crate) blending: bool,
    pub(crate) enabled: bool,
}

impl Default for ObjectColorCalculated {
    fn default() -> Self {
        Self {
            color: Vec4::ONE,
            blending: false,
            enabled: true,
        }
    }
}

pub(crate) fn update_object_color(
    par_commands: ParallelCommands,
    global_sections: Res<GlobalSections>,
    group_archetypes: Query<(Ref<GroupArchetypeCalculated>, Ref<Pulses>)>,
    objects: Query<(
        Entity,
        &ObjectGroups,
        &mut ObjectColor,
        &mut ObjectColorCalculated,
    )>,
    color_channels: Query<Ref<ColorChannelCalculated>>,
) {
    let sections_to_update = &global_sections.sections[global_sections.visible.clone()];

    let compute_task_pool = ComputeTaskPool::get();

    let thread_chunk_size = (sections_to_update.len() / compute_task_pool.thread_num()).max(1);

    let par_commands = &par_commands;
    let objects = &objects;
    let color_channels = &color_channels;
    let group_archetypes = &group_archetypes;

    compute_task_pool.scope(|scope| {
        for thread_chunk in sections_to_update.chunks(thread_chunk_size) {
            scope.spawn(async move {
                let mut color_cache = AHashMap::new();
                for section in thread_chunk {
                    let mut iter = unsafe { objects.iter_many_unsafe(section) };
                    while let Some((entity, object_groups, object_color, mut calculated)) =
                        iter.fetch_next()
                    {
                        if let Some(cached_calculated) = color_cache.get(&(
                            object_groups.archetype_entity,
                            object_color.object_color_kind,
                            object_color.channel_entity,
                        )) {
                            let Some(cached_calculated): &Option<ObjectColorCalculated> =
                                cached_calculated
                            else {
                                continue;
                            };

                            if calculated.blending != cached_calculated.blending {
                                par_commands.command_scope(|mut commands| {
                                    commands.entity(entity).insert(
                                        if !cached_calculated.blending {
                                            Handle::Weak(object_color.texture_ids.0)
                                        } else {
                                            Handle::Weak(object_color.texture_ids.1)
                                        },
                                    );
                                });
                            }

                            *calculated = *cached_calculated;
                            calculated.color[3] *= object_color.object_opacity;
                            continue;
                        }

                        let (group_archetype, pulses) = group_archetypes
                            .get(object_groups.archetype_entity)
                            .unwrap();

                        if !group_archetype.enabled {
                            calculated.bypass_change_detection().enabled = false;
                            color_cache.insert(
                                (
                                    object_groups.archetype_entity,
                                    object_color.object_color_kind,
                                    object_color.channel_entity,
                                ),
                                Some(ObjectColorCalculated {
                                    enabled: false,
                                    ..default()
                                }),
                            );
                            continue;
                        }

                        let (color, blending) = color_channels
                            .get(object_color.channel_entity)
                            .map(|color_channel_calculated| {
                                (
                                    color_channel_calculated.color,
                                    color_channel_calculated.blending,
                                )
                            })
                            .unwrap_or((Vec4::ONE, false));

                        calculated.enabled = true;

                        let mut color = match object_color.object_color_kind {
                            ObjectColorKind::None => Vec4::ONE,
                            ObjectColorKind::Black => Vec3::ZERO.extend(1.),
                            _ => color,
                        };

                        color[3] *= group_archetype.opacity;

                        if object_color.object_color_kind != ObjectColorKind::Black {
                            let iter = pulses
                                .pulses
                                .iter()
                                .filter(|(_, _, target_kind)| {
                                    !(*target_kind != ObjectColorKind::None
                                        && !(*target_kind == ObjectColorKind::Base
                                            && object_color.object_color_kind
                                                == ObjectColorKind::None)
                                        && *target_kind != object_color.object_color_kind)
                                })
                                .map(|(progress, color_mod, _)| (*progress, *color_mod));

                            ColorMod::apply_color_mods(iter, &mut color);
                        }

                        if calculated.blending != blending {
                            par_commands.command_scope(|mut commands| {
                                commands.entity(entity).insert(if !blending {
                                    Handle::Weak(object_color.texture_ids.0)
                                } else {
                                    Handle::Weak(object_color.texture_ids.1)
                                });
                            });
                        }

                        calculated.color = color;
                        calculated.blending = blending;

                        color_cache.insert(
                            (
                                object_groups.archetype_entity,
                                object_color.object_color_kind,
                                object_color.channel_entity,
                            ),
                            Some(*calculated),
                        );

                        calculated.color[3] *= object_color.object_opacity;
                    }
                }
            });
        }
    });
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum ObjectColorKind {
    Base,
    Detail,
    Black,
    #[default]
    None,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ColorMod {
    Color(Vec3A),
    Hsv(HsvMod),
}

impl Default for ColorMod {
    fn default() -> Self {
        Self::Color(Vec3A::ONE)
    }
}

impl ColorMod {
    fn apply_color_mods<I>(mods: I, color: &mut Vec4)
    where
        I: IntoIterator<Item = (f32, Self)>,
    {
        let mut transforming_color = Vec3A::from(*color);
        for (progress, color_mod) in mods {
            match color_mod {
                ColorMod::Color(color) => {
                    if progress == 1. {
                        transforming_color = color;
                        continue;
                    }
                    transforming_color = transforming_color.lerp(color, progress);
                }
                ColorMod::Hsv(hsv_mod) => {
                    let mut target_color = transforming_color;
                    hsv_mod.apply_rgb(&mut target_color);

                    if progress == 1. {
                        transforming_color = target_color;
                        continue;
                    }

                    transforming_color = transforming_color.lerp(target_color, progress);
                }
            }
        }
        *color = transforming_color.extend(color[3]);
    }
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
    pub(crate) fn parse(hsv_string: &str) -> Result<HsvMod, anyhow::Error> {
        let hsv_data: [&str; 5] = de::from_str(hsv_string, 'a')?;
        let h: f32 = hsv_data[0].parse()?;
        let s = hsv_data[1].parse()?;
        let v = hsv_data[2].parse()?;
        let s_absolute = str_to_bool(hsv_data[3]);
        let v_absolute = str_to_bool(hsv_data[4]);

        Ok(HsvMod::new(h * (1. / 360.), s, v, s_absolute, v_absolute))
    }

    pub(crate) fn new(h: f32, s: f32, v: f32, s_absolute: bool, v_absolute: bool) -> Self {
        Self {
            h,
            s,
            v,
            s_absolute,
            v_absolute,
        }
    }

    pub(crate) fn apply_rgba(&self, color: &mut Vec4) {
        let mut applied_rgb = Vec3A::from(*color);
        self.apply_rgb(&mut applied_rgb);
        *color = applied_rgb.extend(color[3]);
    }

    pub(crate) fn apply_rgb(&self, color: &mut Vec3A) {
        let [h, s, v] = rgb_to_hsv(color.to_array());
        let rgb = hsv_to_rgb([
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
        ]);

        *color = Vec3A::from(rgb)
    }
}

impl Default for HsvMod {
    fn default() -> Self {
        HsvMod::new(0., 1., 1., false, false)
    }
}
