use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::time::Instant;

use bevy::app::{App, Main, Plugin, PostUpdate, Startup, Update};
use bevy::core::FrameCountPlugin;
use bevy::log::{info, warn};
use bevy::math::{Quat, Vec3Swizzles};
use bevy::prelude::{
    Commands, Component, default, IntoSystemConfigs, Query, Res, ResMut, Resource, Time, Transform,
    TransformBundle, With, World,
};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::time::TimePlugin;
use bevy::utils::HashMap;
use bevy_enum_filter::prelude::AddEnumFilter;
use futures_lite::future;
use serde::{Deserialize, Deserializer};
use serde::de::Error;

use crate::level::{
    color::ColorKind,
    section::{GlobalSections, Section, update_entity_section, update_global_sections},
};
use crate::utils::{decompress, decrypt};

mod color;
mod de;
mod object;
pub(crate) mod section;

#[derive(Resource)]
pub(crate) enum LevelWorld {
    None,
    Pending(Task<World>),
    World(World),
}

pub(crate) struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_level_world)
            .add_systems(Update, update_level_world);
    }
}

fn spawn_level_world(mut commands: Commands) {
    let async_compute = AsyncComputeTaskPool::get();
    let level_world_future = async_compute.spawn(async move {
        let mut sub_app = App::new();

        sub_app.add_plugins((TimePlugin, FrameCountPlugin));

        sub_app.add_enum_filter::<ColorKind>();

        // sub_app.add_systems(Update, move_test);

        sub_app.add_systems(
            PostUpdate,
            (
                update_entity_section.before(update_global_sections),
                update_global_sections,
            ),
        );

        let mut world = sub_app.world;

        let mut save_file = File::open("assets/khorne.txt").unwrap();
        let mut save_data = Vec::new();
        let _ = save_file.read_to_end(&mut save_data);
        let start_a = Instant::now();
        let decrypted = decrypt::<0>(&save_data).unwrap();
        info!("Decrypt took {:?}", start_a.elapsed());
        let start_a = Instant::now();
        let decompressed = decompress(&decrypted).unwrap();
        info!("Decompression took {:?}", start_a.elapsed());
        let decom_inner_level = DecompressedInnerLevel(decompressed);
        let start_a = Instant::now();
        let parsed = decom_inner_level.parse().unwrap();
        info!("Parsing took {:?}", start_a.elapsed());

        let global_sections = GlobalSections::default();

        let start_a = Instant::now();
        for object_data in &parsed.objects {
            let mut transform = Transform::default();
            if let Some(x) = object_data.get(b"2".as_ref()) {
                transform.translation.x = std::str::from_utf8(x).unwrap().parse().unwrap();
            }
            if let Some(y) = object_data.get(b"3".as_ref()) {
                transform.translation.y = std::str::from_utf8(y).unwrap().parse().unwrap();
            }
            if let Some(rotation) = object_data.get(b"6".as_ref()) {
                transform.rotation = Quat::from_rotation_z(
                    -std::str::from_utf8(rotation)
                        .unwrap()
                        .parse::<f32>()
                        .unwrap()
                        .to_radians(),
                );
            }

            let object_section = Section {
                current: Section::index_from_pos(transform.translation.xy()),
                old: None,
            };

            let global_section_entry = global_sections.0.entry(object_section.current);

            let entity = world
                .spawn(TransformBundle {
                    local: transform,
                    ..default()
                })
                .insert(object_section)
                .insert(MoveMarker)
                .id();

            global_section_entry.or_default().insert(entity);
        }
        info!("Spawning took {:?}", start_a.elapsed());
        info!("Spawned {} objects", parsed.objects.len());
        info!("{} sections used", global_sections.0.len());

        world.insert_resource(global_sections);

        world
    });
    commands.insert_resource(LevelWorld::Pending(level_world_future));
}

fn update_level_world(mut level_world: ResMut<LevelWorld>) {
    match &mut *level_world {
        LevelWorld::World(ref mut world) => {
            world.run_schedule(Main);
            world.clear_trackers();
        }
        LevelWorld::Pending(world_task) => {
            if let Some(world) = future::block_on(future::poll_once(world_task)) {
                *level_world = LevelWorld::World(world);
            }
        }
        _ => (),
    }
}

#[derive(Component)]
struct MoveMarker;

fn move_test(mut ent: Query<&mut Transform, (With<Section>, With<MoveMarker>)>, time: Res<Time>) {
    let val = f32::sin(time.elapsed_seconds() * 20.);
    let val2 = f32::cos(time.elapsed_seconds() * 20.);
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

        let thread_chunk_size = (object_strings.len() - 1) / async_compute.thread_num();
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
                                warn!("Failed to parse object: {}", error);
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
