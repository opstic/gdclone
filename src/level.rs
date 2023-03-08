pub(crate) mod color;
pub(crate) mod de;
pub(crate) mod easing;
pub(crate) mod object;
// pub(crate) mod trigger;

use crate::level::color::{ColorChannel, ColorChannels};
use crate::GameState;
// use crate::level::trigger::{finish_triggers, tick_triggers, TriggerCompleted, TriggerSystems};
use crate::utils::{decompress, decrypt, u8_to_bool};
use bevy::app::{App, CoreStage, Plugin};
use bevy::log::error;
use bevy::prelude::{Commands, Entity, IntoSystemDescriptor, RunCriteriaDescriptorCoercion};
use bevy::utils::HashMap;
use bevy::ecs::schedule::SystemSet;
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use std::io::Read;
use std::marker::PhantomData;

#[derive(Default)]
pub(crate) struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_update(GameState::Play)
                .with_system(object::create_atlas_sprite)
                .with_system(object::create_sprite),
        );
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Level {
    #[serde(rename = "k1")]
    pub(crate) id: Option<u32>,
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

impl<'a> ParsedInnerLevel<'a> {
    pub(crate) fn spawn_level(
        &self,
        commands: &mut Commands,
        low_detail: bool,
    ) -> Result<(), anyhow::Error> {
        let mut colors: HashMap<u64, ColorChannel> = HashMap::with_capacity(75);
        let mut groups: HashMap<u64, Vec<Entity>> =
            HashMap::with_capacity((self.objects.len() / 500).min(500));
        if let Some(colors_string) = self.start_object.get(b"kS38".as_ref()) {
            let parsed_colors: Vec<&[u8]> = de::from_slice(colors_string, b'|')?;
            for color_string in parsed_colors {
                let (index, color) = ColorChannel::parse(color_string)?;
                colors.insert(index, color);
            }
        }
        commands.insert_resource(ColorChannels(colors));
        for object_data in &self.objects {
            if let Some(high_detail) = object_data.get(b"103".as_ref()) {
                if u8_to_bool(high_detail) && low_detail {
                    continue;
                }
            }
            let entity = match object::spawn_object(commands, object_data) {
                Ok(entity) => entity,
                Err(e) => {
                    error!("Error while parsing object: {}", e);
                    continue;
                }
            };
            if let Some(group_string) = object_data.get(b"57".as_ref()) {
                let parsed_groups: Vec<u64> = de::from_slice(group_string, b'.')?;
                for group in parsed_groups {
                    let entry = groups.entry(group);
                    entry.or_default().push(entity);
                }
            }
        }
        Ok(())
    }
}
