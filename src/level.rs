use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::time::Instant;

use bevy::app::{App, First, Last, Plugin, PostUpdate, PreUpdate, RunFixedUpdateLoop, Update};
use bevy::asset::{AssetServer, LoadState};
use bevy::core::FrameCountPlugin;
use bevy::input::Input;
use bevy::log::{info, warn};
use bevy::math::Vec2;
use bevy::prelude::{
    Camera, ClearColor, Commands, Gizmos, IntoSystemConfigs, KeyCode, Local, Mut,
    OrthographicProjection, Query, Res, ResMut, Resource, Time, Transform, With, World,
};
use bevy::render::color::Color;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::time::TimePlugin;
use bevy::utils::{default, HashMap};
use futures_lite::future;
use indexmap::IndexMap;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::asset::cocos2d_atlas::Cocos2dFrames;
use crate::asset::TestAssets;
use crate::level::color::HsvMod;
use crate::level::player::{update_player_pos, Player};
use crate::level::transform::{GlobalTransform2d, Transform2d};
use crate::level::trigger::{
    process_triggers, SpeedChange, TriggerActivator, TriggerSystemStateCache,
};
use crate::level::{
    color::{
        update_color_channel_calculated, update_object_color, ColorChannelCalculated,
        GlobalColorChannel, GlobalColorChannels,
    },
    group::{
        apply_group_delta, clear_group_delta, update_object_group, update_object_group_calculated,
    },
    section::{update_sections, GlobalSections, Section},
    transform::update_transform,
};
use crate::utils::{decompress, decrypt, section_index_from_x, U64Hash};

pub(crate) mod color;
mod de;
mod easing;
pub(crate) mod group;
pub(crate) mod object;
mod player;
pub(crate) mod section;
pub(crate) mod transform;
pub(crate) mod trigger;

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
            .init_resource::<Options>()
            .add_systems(Update, (update_level_world, update_controls))
            .add_systems(PostUpdate, spawn_level_world);
    }
}

fn spawn_level_world(
    cocos2d_frames: Res<Cocos2dFrames>,
    server: Res<AssetServer>,
    test_assets: Res<TestAssets>,
    mut level_world: ResMut<LevelWorld>,
    mut a: Local<bool>,
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

    if !*a {
        *a = true;
        return;
    }

    let async_compute = AsyncComputeTaskPool::get();

    let cocos2d_frames = cocos2d_frames.clone();
    let level_world_future = async_compute.spawn(async move {
        let mut sub_app = App::new();

        sub_app.add_plugins((TimePlugin, FrameCountPlugin));

        sub_app.add_systems(PreUpdate, clear_group_delta);

        sub_app.add_systems(
            Update,
            (update_player_pos, process_triggers.after(update_player_pos)),
        );

        sub_app.add_systems(
            PostUpdate,
            (
                update_object_group,
                update_object_group_calculated.after(update_object_group),
                update_color_channel_calculated,
                apply_group_delta.before(update_sections),
                update_sections,
                update_transform.after(update_sections),
                update_object_color
                    .after(update_sections)
                    .after(update_object_group_calculated)
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

        global_color_channels.0.insert(
            1007,
            world
                .spawn((
                    GlobalColorChannel::Copy {
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
                    ColorChannelCalculated::default(),
                ))
                .id(),
        );

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

        color::construct_color_channel_hierarchy(&mut world, &mut global_color_channels);
        info!("Color channel parsing took {:?}", start.elapsed());

        let mut global_sections = GlobalSections::default();
        let mut global_groups = IndexMap::with_hasher(U64Hash);

        start = Instant::now();

        // Spawn the objects in order of the sections to hopefully improve access pattern
        let mut temp_objects = Vec::with_capacity(parsed.objects.len());
        for (index, object_data) in parsed.objects.iter().enumerate() {
            let object_position = object::get_object_pos(object_data).unwrap();
            temp_objects.push((object_position.x, index as u32));
        }

        radsort::sort_by_key(&mut temp_objects, |temp| temp.0);

        for (_, index) in temp_objects {
            if let Err(error) = object::spawn_object(
                &mut world,
                &parsed.objects[index as usize],
                &mut global_sections,
                &mut global_groups,
                &global_color_channels,
                &cocos2d_frames,
            ) {
                warn!("Failed to spawn object: {:?}", error);
            }
        }

        // for object_data in &parsed.objects {
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
        info!("Spawned {} objects", parsed.objects.len());
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

        start = Instant::now();
        group::spawn_groups(&mut world, global_groups);
        info!("Initializing groups took {:?}", start.elapsed());

        let default_speed = if let Some(speed) = parsed.start_object.get(b"kA4".as_ref()) {
            match std::str::from_utf8(speed).unwrap().parse().unwrap() {
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

        world.init_resource::<TriggerSystemStateCache>();

        info!("Total time: {:?}", start_all.elapsed());

        world
    });

    *level_world = LevelWorld::Pending(level_world_future);
}

#[derive(Resource)]
struct Options {
    synchronize_cameras: bool,
    display_simulated_camera: bool,
    visible_sections_from_simulated: bool,
    show_lines: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            synchronize_cameras: true,
            display_simulated_camera: false,
            visible_sections_from_simulated: false,
            show_lines: true,
        }
    }
}

fn update_controls(
    mut projections: Query<&mut OrthographicProjection, With<Camera>>,
    mut transforms: Query<&mut Transform, With<Camera>>,
    keys: Res<Input<KeyCode>>,
    mut options: ResMut<Options>,
    time: Res<Time>,
) {
    let multiplier = time.delta_seconds() * 20.;
    if keys.just_pressed(KeyCode::U) {
        options.synchronize_cameras = !options.synchronize_cameras;
    }
    if keys.just_pressed(KeyCode::L) {
        options.show_lines = !options.show_lines;
    }
    for mut transform in transforms.iter_mut() {
        if !options.synchronize_cameras {
            if keys.pressed(KeyCode::Right) {
                transform.translation.x += 10.0 * multiplier;
            }
            if keys.pressed(KeyCode::Left) {
                transform.translation.x -= 10.0 * multiplier;
            }
            if keys.pressed(KeyCode::A) {
                transform.translation.x -= 20.0 * multiplier;
            }
            if keys.pressed(KeyCode::D) {
                transform.translation.x += 20.0 * multiplier;
            }
        }
        if keys.pressed(KeyCode::Up) {
            transform.translation.y += 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::Down) {
            transform.translation.y -= 10.0 * multiplier;
        }
        if keys.pressed(KeyCode::W) {
            transform.translation.y += 20.0 * multiplier;
        }
        if keys.pressed(KeyCode::S) {
            transform.translation.y -= 20.0 * multiplier;
        }
    }
    for mut projection in projections.iter_mut() {
        if keys.pressed(KeyCode::Q) {
            projection.scale *= 1.01;
        }
        if keys.pressed(KeyCode::E) {
            projection.scale *= 0.99;
        }
    }
}

fn update_level_world(
    mut commands: Commands,
    mut camera: Query<(&OrthographicProjection, &mut Transform)>,
    mut level_world: ResMut<LevelWorld>,
    options: Res<Options>,
    mut gizmos: Gizmos,
) {
    match &mut *level_world {
        LevelWorld::World(ref mut world) => {
            world.run_schedule(First);
            world.run_schedule(PreUpdate);
            world.run_schedule(RunFixedUpdateLoop);
            world.run_schedule(Update);

            // Render player line
            let mut players = world.query::<(&Player, &Transform2d)>();

            if options.show_lines {
                for (player, transform) in players.iter(world) {
                    let (player_line_start, player_line_end) = if player.vertical_is_x {
                        (
                            Vec2::new(transform.translation.x - 500., transform.translation.y),
                            Vec2::new(transform.translation.x + 500., transform.translation.y),
                        )
                    } else {
                        (
                            Vec2::new(transform.translation.x, transform.translation.y - 500.),
                            Vec2::new(transform.translation.x, transform.translation.y + 500.),
                        )
                    };
                    gizmos.line_2d(player_line_start, player_line_end, Color::ORANGE_RED)
                }
            }

            let (camera_projection, mut camera_transform) = camera.single_mut();

            let (_, player_transform) = players.single(world);

            if options.synchronize_cameras {
                camera_transform.translation.x = player_transform.translation.x + 75.;
                if options.show_lines {
                    gizmos.line_2d(
                        Vec2::new(
                            camera_transform.translation.x,
                            camera_transform.translation.y - 500.,
                        ),
                        Vec2::new(
                            camera_transform.translation.x,
                            camera_transform.translation.y + 500.,
                        ),
                        Color::GREEN,
                    );
                    gizmos.line_2d(
                        Vec2::new(
                            player_transform.translation.x,
                            camera_transform.translation.y - 500.,
                        ),
                        Vec2::new(
                            player_transform.translation.x,
                            camera_transform.translation.y + 500.,
                        ),
                        Color::ORANGE_RED,
                    );
                }
            }

            let camera_min = camera_projection.area.min.x + camera_transform.translation.x;
            let camera_max = camera_projection.area.max.x + camera_transform.translation.x;

            let min_section = section_index_from_x(camera_min) as usize;
            let max_section = section_index_from_x(camera_max) as usize;

            let mut global_sections = world.resource_mut::<GlobalSections>();
            global_sections.visible = min_section.saturating_sub(2)..max_section.saturating_add(2);

            world.run_schedule(PostUpdate);
            world.run_schedule(Last);

            world.resource_scope(|world, global_color_channels: Mut<GlobalColorChannels>| {
                if let Some(entity) = global_color_channels.0.get(&1000) {
                    let mut query = world.query::<&ColorChannelCalculated>();
                    if let Ok(calculated) = query.get(world, *entity) {
                        commands.insert_resource(ClearColor(calculated.color));
                    }
                }
            });

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
