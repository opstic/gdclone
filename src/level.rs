pub(crate) mod color;
pub(crate) mod de;
pub(crate) mod easing;
pub(crate) mod object;
pub(crate) mod trigger;

use crate::level::color::{BaseColor, ColorChannel, ColorChannels, CopyColor, Hsv};
use crate::level::trigger::TriggerSystems;
use crate::utils::{decompress, decrypt, u8_to_bool, PassHashMap};
use crate::GameState;
use bevy::app::{App, CoreSet, Plugin};

use bevy::log::error;
use bevy::prelude::{Color, Commands, Entity, IntoSystemConfig, OnUpdate, Resource};
use bevy::utils::{hashbrown, HashMap, HashSet, PassHash};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::level::object::Object;
use crate::loaders::cocos2d_atlas::{Cocos2dAtlas, Cocos2dFrames};
use bevy::asset::Assets;
use bevy::math::IVec2;
use bevy::render::view;
use bevy::render::view::VisibilitySystems;
use std::marker::PhantomData;

#[derive(Default)]
pub(crate) struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(
            trigger::activate_xpos_triggers
                .in_set(TriggerSystems::ActivateTriggers)
                .in_set(OnUpdate(GameState::Play)),
        )
        .add_system(
            trigger::execute_triggers
                .in_set(TriggerSystems::ExecuteTriggers)
                .after(TriggerSystems::ActivateTriggers),
        )
        .add_system(
            object::update_visibility
                .in_set(VisibilitySystems::CheckVisibility)
                .after(view::check_visibility),
        )
        .add_system(
            object::propagate_visibility
                .after(object::update_visibility)
                .in_set(VisibilitySystems::CheckVisibility),
        )
        .add_system(
            color::calculate_object_color
                .after(object::propagate_visibility)
                .in_base_set(CoreSet::PostUpdate),
        )
        .register_type::<Object>()
        .init_resource::<ColorChannels>()
        .init_resource::<Groups>()
        .init_resource::<Sections>()
        .init_resource::<trigger::ExecutingTriggers>();
    }
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

fn decrypt_inner_level<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer).unwrap();
    Ok(Some(decrypt(s.as_bytes(), None).map_err(Error::custom)?))
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

pub(crate) struct DecompressedInnerLevel(Vec<u8>);

impl DecompressedInnerLevel {
    pub(crate) fn parse(&self) -> Result<ParsedInnerLevel, anyhow::Error> {
        let object_strings: Vec<&[u8]> = de::from_slice(&self.0, b';')?;
        let mut objects = Vec::with_capacity(object_strings.len() - 1);
        let start_object: HashMap<&[u8], &[u8]> =
            de::from_slice(object_strings.first().unwrap(), b',')?;
        for object_string in &object_strings[1..] {
            let object: HashMap<&[u8], &[u8]> = de::from_slice(object_string, b',')?;
            objects.push(object);
        }
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

#[derive(Default, Resource)]
pub(crate) struct Groups(pub(crate) PassHashMap<Group>);

pub(crate) struct Group {
    pub(crate) entities: Vec<Entity>,
    pub(crate) activated: bool,
    pub(crate) opacity: f32,
}

impl Default for Group {
    fn default() -> Self {
        Group {
            entities: Vec::new(),
            activated: true,
            opacity: 1.,
        }
    }
}

pub(crate) const SECTION_SIZE: f32 = 100.;

#[derive(Default, Resource)]
pub(crate) struct Sections(pub(crate) HashMap<IVec2, HashSet<Entity>>);

impl Sections {
    pub(crate) fn get_section(&self, index: &IVec2) -> Option<&HashSet<Entity>> {
        self.0.get(index)
    }

    pub(crate) fn get_section_mut(&mut self, index: &IVec2) -> &mut HashSet<Entity> {
        self.0.entry(*index).or_default()
    }
}

impl<'a> ParsedInnerLevel<'a> {
    pub(crate) fn objects(&self) -> usize {
        self.objects.len()
    }

    pub(crate) fn spawn_level(
        &self,
        commands: &mut Commands,
        sections: &mut Sections,
        cocos2d_frames: &Cocos2dFrames,
        cocos2d_atlases: &Assets<Cocos2dAtlas>,
        low_detail: bool,
    ) -> Result<(), anyhow::Error> {
        sections.0.clear();
        let mut colors: PassHashMap<ColorChannel> = hashbrown::HashMap::with_hasher(PassHash);
        let mut groups: PassHashMap<Group> = hashbrown::HashMap::with_capacity_and_hasher(
            (self.objects.len() / 500).min(500),
            PassHash,
        );
        if let Some(colors_string) = self.start_object.get(b"kS38".as_ref()) {
            let parsed_colors: Vec<&[u8]> = de::from_slice(colors_string, b'|')?;
            colors.reserve(parsed_colors.len().saturating_sub(colors.capacity()));
            for color_string in parsed_colors {
                let (index, color) = ColorChannel::parse(color_string)?;
                colors.insert(index, color);
            }
        }
        colors.insert(
            1007,
            ColorChannel::CopyColor(CopyColor {
                copied_index: 1000,
                opacity: 1.0,
                blending: true,
                hsv: Hsv {
                    s: -20.,
                    s_absolute: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
        colors.insert(
            1010,
            ColorChannel::BaseColor(BaseColor {
                color: Color::BLACK,
                blending: false,
            }),
        );
        commands.insert_resource(ColorChannels(colors));
        for object_data in &self.objects {
            if let Some(high_detail) = object_data.get(b"103".as_ref()) {
                if u8_to_bool(high_detail) && low_detail {
                    continue;
                }
            }
            let parsed_groups: Vec<u64> =
                if let Some(group_string) = object_data.get(b"57".as_ref()) {
                    de::from_slice(group_string, b'.')?
                } else {
                    Vec::new()
                };
            let entity = match object::spawn_object(
                commands,
                object_data,
                parsed_groups.clone(),
                sections,
                cocos2d_frames,
                cocos2d_atlases,
            ) {
                Ok(entity) => entity,
                Err(e) => {
                    error!("Error while parsing object: {}", e);
                    continue;
                }
            };
            for group in parsed_groups {
                let entry = groups.entry(group).or_default();
                entry.entities.push(entity);
            }
        }
        commands.insert_resource(Groups(groups));
        Ok(())
    }
}
