use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::time::Instant;

use bevy::app::{App, Main, Plugin, PostUpdate, PreUpdate, Update};
use bevy::asset::{AssetServer, LoadState};
use bevy::core::FrameCountPlugin;
use bevy::log::{info, warn};
use bevy::prelude::{
    Component, IntoSystemConfigs, Query, Res, ResMut, Resource, Time, Transform, With, World,
};
use bevy::render::color::Color;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::time::TimePlugin;
use bevy::utils::HashMap;
use bevy_enum_filter::prelude::AddEnumFilter;
use futures_lite::future;
use indexmap::IndexMap;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::asset::cocos2d_atlas::Cocos2dFrames;
use crate::asset::TestAssets;
use crate::level::{
    color::{
        update_color_channel_calculated, update_object_color, ColorChannelCalculated,
        GlobalColorChannel, GlobalColorChannels, ObjectColorKind,
    },
    group::clear_group_delta,
    section::{
        propagate_section_change, update_entity_section, update_global_sections, GlobalSections,
        Section, VisibleGlobalSections,
    },
    transform::update_transform,
};
use crate::utils::{decompress, decrypt, U64Hash};

pub(crate) mod color;
mod de;
mod group;
pub(crate) mod object;
pub(crate) mod section;
mod transform;

#[derive(Default, Resource)]
pub(crate) enum LevelWorld {
    #[default]
    None,
    Pending(Task<World>),
    World(World),
}

pub(crate) struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelWorld>()
            .add_systems(Update, update_level_world)
            .add_systems(PostUpdate, spawn_level_world);
    }
}

fn spawn_level_world(
    cocos2d_frames: Res<Cocos2dFrames>,
    server: Res<AssetServer>,
    test_assets: Res<TestAssets>,
    mut level_world: ResMut<LevelWorld>,
) {
    match *level_world {
        LevelWorld::None => (),
        _ => return,
    }

    for handle in &test_assets.assets {
        if server.load_state(handle) != LoadState::Loaded {
            return;
        }
    }

    let async_compute = AsyncComputeTaskPool::get();

    let cocos2d_frames = cocos2d_frames.clone();
    let level_world_future = async_compute.spawn(async move {
        let mut sub_app = App::new();

        sub_app.add_plugins((TimePlugin, FrameCountPlugin));

        sub_app.add_enum_filter::<ObjectColorKind>();

        sub_app.add_systems(PreUpdate, clear_group_delta);

        sub_app.add_systems(Update, move_test);

        sub_app.add_systems(
            PostUpdate,
            (
                update_color_channel_calculated,
                update_entity_section.before(update_global_sections),
                propagate_section_change
                    .after(update_entity_section)
                    .before(update_global_sections),
                update_global_sections,
                update_transform.after(update_global_sections),
                update_object_color
                    .after(update_global_sections)
                    .after(update_color_channel_calculated),
            ),
        );

        let mut world = sub_app.world;

        let mut save_file = File::open("assets/theeschaton.txt").unwrap();
        let mut save_data = Vec::new();
        let _ = save_file.read_to_end(&mut save_data);
        let start_all = Instant::now();
        let decrypted = decrypt::<0>(&save_data).unwrap();
        info!("Decrypting took {:?}", start_all.elapsed());
        let mut start = Instant::now();
        let decompressed = decompress(&decrypted).unwrap();
        info!("Decompressing took {:?}", start.elapsed());
        let decom_inner_level = DecompressedInnerLevel(decompressed);
        start = Instant::now();
        let parsed = decom_inner_level.parse().unwrap();
        info!("Parsing took {:?}", start.elapsed());

        let mut global_color_channels = GlobalColorChannels::default();

        global_color_channels.0.insert(
            1010,
            world
                .spawn((
                    GlobalColorChannel::Base {
                        color: Color::BLACK,
                        blending: false,
                    },
                    ColorChannelCalculated::default(),
                ))
                .id(),
        );

        start = Instant::now();
        if let Some(colors_string) = parsed.start_object.get(b"kS38".as_ref()) {
            let parsed_colors: Vec<&[u8]> = de::from_slice(colors_string, b'|').unwrap();
            global_color_channels
                .0
                .try_reserve(parsed_colors.len())
                .unwrap();
            for color_string in parsed_colors {
                let (index, color_channel) = match GlobalColorChannel::parse(color_string) {
                    Ok(result) => result,
                    Err(error) => {
                        warn!("Failed to parse color channel: {:?}", error);
                        continue;
                    }
                };
                let color_channel_entity = world
                    .spawn((color_channel, ColorChannelCalculated::default()))
                    .id();

                global_color_channels.0.insert(index, color_channel_entity);
            }
        }
        color::construct_color_channel_hierarchy(&mut world, &mut global_color_channels);
        info!("Color channel parsing took {:?}", start.elapsed());

        let global_sections = GlobalSections::default();
        let mut global_groups = IndexMap::with_hasher(U64Hash);

        start = Instant::now();
        for object_data in &parsed.objects {
            if let Err(error) = object::spawn_object(
                &mut world,
                object_data,
                &global_sections,
                &mut global_groups,
                &global_color_channels,
                &cocos2d_frames,
            ) {
                warn!("Failed to spawn object: {:?}", error);
            }
        }
        info!("Spawning took {:?}", start.elapsed());
        info!("Spawned {} objects", parsed.objects.len());
        info!("{} sections used", global_sections.0.len());

        world.insert_resource(global_sections);
        world.insert_resource(VisibleGlobalSections { x: 0..6, y: 0..8 });
        world.insert_resource(global_color_channels);

        start = Instant::now();
        group::spawn_groups(&mut world, global_groups);
        info!("Initializing groups took {:?}", start.elapsed());

        info!("Total time: {:?}", start_all.elapsed());

        world
    });

    *level_world = LevelWorld::Pending(level_world_future);
}

fn update_level_world(mut level_world: ResMut<LevelWorld>) {
    match &mut *level_world {
        LevelWorld::World(ref mut world) => {
            world.clear_trackers();
            world.run_schedule(Main);
        }
        LevelWorld::Pending(world_task) => {
            if let Some(mut world) = future::block_on(future::poll_once(world_task)) {
                world.run_schedule(Main);
                *level_world = LevelWorld::World(world);
            }
        }
        _ => (),
    }
}

#[derive(Component)]
struct MoveMarker;

fn move_test(mut ent: Query<&mut Transform, (With<Section>, With<MoveMarker>)>, time: Res<Time>) {
    let val = f32::sin(time.elapsed_seconds() * 10.);
    let val2 = f32::cos(time.elapsed_seconds() * 10.);
    ent.par_iter_mut().for_each(|mut transform| {
        transform.translation.x += val;
        transform.translation.y += val2;
    })
}

struct Group {}

fn decrypt_inner_level<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer).unwrap();
    Ok(Some(decrypt::<0>(s.as_bytes()).map_err(Error::custom)?))
}

#[derive(Debug, Deserialize)]
pub(crate) struct Level {
    #[serde(rename = "k1")]
    pub(crate) id: Option<u64>,
    #[serde(rename = "k2")]
    pub(crate) name: String,
    #[serde(rename = "k3")]
    pub(crate) description: Option<String>,
    #[serde(default, rename = "k4", deserialize_with = "decrypt_inner_level")]
    pub(crate) inner_level: Option<Vec<u8>>,
    #[serde(rename = "k5")]
    pub(crate) creator: String,
}

impl Level {
    pub(crate) fn decompress_inner_level(
        &self,
    ) -> Option<Result<DecompressedInnerLevel, anyhow::Error>> {
        self.inner_level
            .as_ref()
            .map(|compressed| Ok(DecompressedInnerLevel(decompress(compressed)?)))
    }
}

pub(crate) struct DecompressedInnerLevel(pub(crate) Vec<u8>);

impl DecompressedInnerLevel {
    pub(crate) fn parse(&self) -> Result<ParsedInnerLevel, anyhow::Error> {
        let object_strings: Vec<&[u8]> = de::from_slice(&self.0, b';')?;

        if object_strings.is_empty() {
            return Ok(ParsedInnerLevel {
                start_object: HashMap::default(),
                objects: Vec::default(),
                phantom: PhantomData,
            });
        }

        let start_object: HashMap<&[u8], &[u8]> = de::from_slice(object_strings[0], b',')?;

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
                        match de::from_slice(object_string, b',') {
                            Ok(parsed) => *parsed_object = parsed,
                            Err(error) => {
                                warn!("Failed to parse object: {:?}", error);
                                warn!(
                                    "Failed object string: {}",
                                    std::str::from_utf8(object_string).unwrap()
                                );
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
    start_object: HashMap<&'a [u8], &'a [u8]>,
    objects: Vec<HashMap<&'a [u8], &'a [u8]>>,
    phantom: PhantomData<&'a DecompressedInnerLevel>,
}
