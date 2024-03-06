use std::marker::PhantomData;
use std::time::Instant;

use bevy::app::{App, PostUpdate, PreUpdate, Update};
use bevy::core::FrameCountPlugin;
use bevy::log::{info, warn};
use bevy::math::Vec3;
use bevy::prelude::{IntoSystemConfigs, Resource, World};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::time::TimePlugin;
use bevy::utils::{default, HashMap};
use indexmap::IndexMap;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::asset::cocos2d_atlas::Cocos2dFrames;
use crate::level::color::{GlobalColorChannelKind, HsvMod, Pulses};
use crate::level::player::{update_player_pos, Player};
use crate::level::transform::{GlobalTransform2d, Transform2d};
use crate::level::trigger::{process_triggers, SpeedChange, TriggerActivator, TriggerData};
use crate::level::{
    color::{
        clear_pulses, update_color_channel_calculated, update_object_color, ColorChannelCalculated,
        GlobalColorChannel, GlobalColorChannels,
    },
    group::{
        apply_group_delta, clear_group_delta, update_group_archetype,
        update_group_archetype_calculated,
    },
    section::{limit_sections, update_sections, GlobalSections, Section},
    transform::update_transform,
};
use crate::utils::{decompress, decrypt, U64Hash};

pub(crate) mod collision;
pub(crate) mod color;
pub(crate) mod de;
mod easing;
pub(crate) mod group;
pub(crate) mod object;
pub(crate) mod player;
pub(crate) mod section;
pub(crate) mod transform;
pub(crate) mod trigger;

#[derive(Default, Resource)]
pub(crate) enum LevelWorld {
    #[default]
    None,
    Pending(Task<Result<World, anyhow::Error>>),
    World(World),
}

fn base64_decrypt<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer).unwrap();
    Ok(Some(decrypt::<0>(s.as_bytes()).map_err(Error::custom)?))
}

fn base64_decrypt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer).unwrap();
    Ok(Some(
        String::from_utf8(decrypt::<0>(s.as_bytes()).map_err(Error::custom)?)
            .map_err(|err| Error::custom(err.to_string()))?,
    ))
}

fn decode_percent<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer).unwrap();
    Ok(percent_encoding::percent_decode_str(&s)
        .decode_utf8()
        .map_err(|err| Error::custom(err.to_string()))?
        .to_string())
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct LevelInfo {
    #[serde(rename = "1")]
    pub(crate) id: u64,
    #[serde(rename = "2")]
    pub(crate) name: String,
    #[serde(rename = "35")]
    pub(crate) song_id: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SongInfo {
    #[serde(rename = "1")]
    pub(crate) id: u64,
    #[serde(rename = "2")]
    pub(crate) name: String,
    #[serde(rename = "10", deserialize_with = "decode_percent")]
    pub(crate) url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LevelData {
    #[serde(alias = "k1", alias = "1")]
    pub(crate) id: Option<u64>,
    #[serde(alias = "k2", alias = "2")]
    pub(crate) name: String,
    #[serde(alias = "k3", alias = "3", deserialize_with = "base64_decrypt_string")]
    pub(crate) description: Option<String>,
    #[serde(
        default,
        alias = "k4",
        alias = "4",
        deserialize_with = "base64_decrypt"
    )]
    pub(crate) inner_level: Option<Vec<u8>>,
    #[serde(rename = "k5")]
    pub(crate) creator: Option<String>,
}

impl LevelData {
    pub(crate) fn decompress_inner_level(
        &self,
    ) -> Option<Result<DecompressedInnerLevel, anyhow::Error>> {
        self.inner_level.as_ref().map(|compressed| {
            let decompressed = decompress(compressed)?;
            // Validate the data
            simdutf8::basic::from_utf8(&decompressed)?;
            Ok(DecompressedInnerLevel(unsafe {
                String::from_utf8_unchecked(decompressed)
            }))
        })
    }
}

pub(crate) struct DecompressedInnerLevel(pub(crate) String);

impl DecompressedInnerLevel {
    pub(crate) fn parse(&self) -> Result<ParsedInnerLevel, anyhow::Error> {
        let object_strings: Vec<&str> = de::from_str(&self.0, ';')?;

        if object_strings.is_empty() {
            return Ok(ParsedInnerLevel {
                start_object: HashMap::default(),
                objects: Vec::default(),
                phantom: PhantomData,
            });
        }

        let start_object: HashMap<&str, &str> = de::from_str(object_strings[0], ',')?;

        let mut objects = vec![HashMap::new(); object_strings.len() - 1];

        let async_compute = AsyncComputeTaskPool::get();

        let thread_chunk_size = ((object_strings.len() - 1) / async_compute.thread_num()).max(1);

        async_compute.scope(|scope| {
            for (object_strings_chunk, parsed_object_chunk) in object_strings[1..]
                .chunks(thread_chunk_size)
                .zip(objects.chunks_mut(thread_chunk_size))
            {
                scope.spawn(async move {
                    for (object_string, parsed_object) in
                        object_strings_chunk.iter().zip(parsed_object_chunk)
                    {
                        match de::from_str(object_string, ',') {
                            Ok(parsed) => *parsed_object = parsed,
                            Err(error) => {
                                warn!("Failed to parse object: {:?}", error);
                                warn!("Failed object string: {}", object_string);
                            }
                        }
                    }
                });
            }
        });

        Ok(ParsedInnerLevel {
            start_object,
            objects,
            phantom: PhantomData,
        })
    }
}

#[derive(Debug)]
pub(crate) struct ParsedInnerLevel<'a> {
    start_object: HashMap<&'a str, &'a str>,
    objects: Vec<HashMap<&'a str, &'a str>>,
    phantom: PhantomData<&'a DecompressedInnerLevel>,
}

impl<'a> ParsedInnerLevel<'a> {
    pub(crate) fn create_world(&self, cocos2d_frames: &Cocos2dFrames) -> World {
        let mut sub_app = App::new();

        sub_app.add_plugins((TimePlugin, FrameCountPlugin));

        sub_app.add_systems(PreUpdate, clear_group_delta);

        sub_app.add_systems(
            Update,
            (
                (update_player_pos, clear_pulses).before(process_triggers),
                process_triggers.after(update_player_pos),
                (
                    update_group_archetype,
                    update_group_archetype_calculated.after(update_group_archetype),
                    update_color_channel_calculated,
                    apply_group_delta,
                    update_sections.after(apply_group_delta),
                )
                    .after(process_triggers),
            ),
        );

        sub_app.add_systems(
            PostUpdate,
            (
                limit_sections,
                (update_transform, update_object_color).after(limit_sections),
            ),
        );

        let mut world = sub_app.world;

        let mut global_color_channels = GlobalColorChannels::default();

        let mut start = Instant::now();
        if let Some(colors_string) = self.start_object.get("kS38") {
            let parsed_colors: Vec<&str> = de::from_str(colors_string, '|').unwrap();
            global_color_channels
                .0
                .try_reserve(parsed_colors.len())
                .unwrap();
            for color_string in parsed_colors {
                let color_channel = match GlobalColorChannel::parse(color_string) {
                    Ok(result) => result,
                    Err(error) => {
                        warn!("Failed to parse color channel: {:?}", error);
                        continue;
                    }
                };

                let id = color_channel.id;

                let color_channel_entity = world
                    .spawn((
                        color_channel,
                        ColorChannelCalculated::default(),
                        Pulses::default(),
                    ))
                    .id();

                global_color_channels.0.insert(id, color_channel_entity);
            }
        }

        global_color_channels.0.insert(
            1007,
            world
                .spawn((
                    GlobalColorChannel {
                        id: 1007,
                        kind: GlobalColorChannelKind::Copy {
                            copied_index: 1000,
                            copy_opacity: false,
                            opacity: 1.,
                            blending: true,
                            hsv: Some(HsvMod {
                                s: -20.,
                                s_absolute: true,
                                ..default()
                            }),
                        },
                    },
                    ColorChannelCalculated::default(),
                    Pulses::default(),
                ))
                .id(),
        );

        global_color_channels.0.insert(
            1010,
            world
                .spawn((
                    GlobalColorChannel {
                        id: 1010,
                        kind: GlobalColorChannelKind::Base {
                            color: Vec3::ZERO.extend(1.),
                            blending: false,
                        },
                    },
                    ColorChannelCalculated::default(),
                    Pulses::default(),
                ))
                .id(),
        );

        for i in 0..1050 {
            if global_color_channels.0.contains_key(&i) {
                continue;
            }

            global_color_channels.0.insert(
                i,
                world
                    .spawn((
                        GlobalColorChannel {
                            id: i,
                            kind: GlobalColorChannelKind::default(),
                        },
                        ColorChannelCalculated::default(),
                        Pulses::default(),
                    ))
                    .id(),
            );
        }

        color::construct_color_channel_hierarchy(&mut world, &mut global_color_channels);
        info!("Color channel parsing took {:?}", start.elapsed());

        let mut global_sections = GlobalSections::default();
        let mut global_groups = IndexMap::with_hasher(U64Hash);
        let mut group_archetypes = IndexMap::new();

        start = Instant::now();

        // Spawn the objects in order of the sections to hopefully improve access pattern
        let mut temp_objects = Vec::with_capacity(self.objects.len());
        for (index, object_data) in self.objects.iter().enumerate() {
            let object_position = object::get_object_pos(object_data).unwrap();
            temp_objects.push((
                (object_position.x, object_position.y, object_position.z),
                index as u32,
            ));
        }

        radsort::sort_by_key(&mut temp_objects, |temp| temp.0);

        for (_, index) in temp_objects {
            if let Err(error) = object::spawn_object(
                &mut world,
                &self.objects[index as usize],
                &mut global_sections,
                &mut global_groups,
                &mut group_archetypes,
                &global_color_channels,
                &cocos2d_frames,
            ) {
                warn!("Failed to spawn object: {:?}", error);
            }
        }

        // for object_data in &selfobjects {
        //     if let Err(error) = object::spawn_object(
        //         &mut world,
        //         object_data,
        //         &global_sections,
        //         &mut global_groups,
        //         &global_color_channels,
        //         &cocos2d_frames,
        //     ) {
        //         warn!("Failed to spawn object: {:?}", error);
        //     }
        // }
        info!("Spawning took {:?}", start.elapsed());
        info!("Spawned {} objects", self.objects.len());
        info!("{} sections used", global_sections.sections.len());

        let player = world
            .spawn((
                Player::default(),
                Transform2d::default(),
                GlobalTransform2d::default(),
                Section::default(),
                TriggerActivator::default(),
            ))
            .id();

        global_sections.sections[0].insert(player);

        world.insert_resource(global_sections);
        world.insert_resource(global_color_channels);

        info!("Found {} group archetypes", group_archetypes.len());
        start = Instant::now();
        group::spawn_groups(&mut world, global_groups, group_archetypes);
        info!("Initializing groups took {:?}", start.elapsed());

        let default_speed = if let Some(speed) = self.start_object.get("kA4") {
            match speed.parse().unwrap() {
                0 => (5.77 * 60., 0.9),
                1 => (5.98 * 60., 0.7),
                2 => (5.87 * 60., 1.1),
                3 => (6. * 60., 1.3),
                4 => (6. * 60., 1.6),
                _ => (5.77 * 60., 0.9),
            }
        } else {
            (5.77 * 60., 0.9)
        };

        world.spawn((
            Transform2d::default(),
            GlobalTransform2d::default(),
            SpeedChange {
                forward_velocity: default_speed.0,
                speed: default_speed.1,
            },
        ));

        start = Instant::now();
        trigger::construct_trigger_index(&mut world);
        info!("Trigger timeline construction took {:?}", start.elapsed());

        world.init_resource::<TriggerData>();

        world
    }
}
