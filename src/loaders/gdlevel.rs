use aho_corasick::AhoCorasick;
use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::BoxedFuture;
use plist::Dictionary;
use serde::Deserialize;
use std::io::Read;
use bevy_prototype_lyon::prelude::tess::path::AttributeStore;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "1303d57b-af74-4318-ac9b-5d9e5519bcf1"]
pub struct GDSaveFile {
    pub(crate) levels: Vec<GDLevel>,
}

#[derive(Debug, Deserialize)]
pub struct GDLevel {
    pub(crate) id: Option<u64>,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) inner_level: Vec<GDLevelObject>,
}

#[derive(Debug, Deserialize)]
pub struct GDLevelObject {
    pub(crate) id: u16,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) rot: f32,
    pub(crate) scale: f32,
}

#[derive(Default)]
pub struct GDSaveLoader;

impl AssetLoader for GDSaveLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            info!("Loading save");
            let decrypted = decrypt(bytes, Some(11_u8))?;
            let fixed = AhoCorasick::new(FIX_PATTERN).replace_all_bytes(&decrypted, FIX_REPLACE);
            let plist: Dictionary = plist::from_bytes(&fixed)?;
            let mut levels: Vec<GDLevel> = Vec::new();
            for (key_name, key) in plist.get("LLM_01").unwrap().as_dictionary().unwrap() {
                if key_name == "_isArr" {
                    continue;
                }
                let level = key.as_dictionary().unwrap();
                info!("Loading {}", level.get("k2").unwrap().as_string().unwrap().to_string());
                let level_id = if let Some(id) = level.get("k1") {
                    Some(id.as_unsigned_integer().unwrap())
                } else {
                    None
                };
                let level_description = if let Some(description) = level.get("k3") {
                    Some(description.as_string().unwrap().to_string())
                } else {
                    None
                };
                let level_inner = if let Some(inner) = level.get("k4") {
                    decode_inner_level(&decrypt(inner.as_string().unwrap().as_bytes(), None)?)?
                } else {
                    Vec::new()
                };
                levels.push(GDLevel {
                    id: level_id,
                    name: level.get("k2").unwrap().as_string().unwrap().to_string(),
                    description: level_description,
                    inner_level: level_inner,
                });
            }
            load_context.set_default_asset(LoadedAsset::new(GDSaveFile { levels }));
            info!("Done");
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["dat"]
    }
}

fn decrypt(bytes: &[u8], key: Option<u8>) -> Result<Vec<u8>, bevy::asset::Error> {
    let nul_byte_start = bytes
        .iter()
        .rposition(|byte| *byte != 11_u8)
        .unwrap_or(bytes.len() - 1);
    let xor: Vec<u8> = match key {
        Some(key) => bytes[..nul_byte_start + 1]
            .iter()
            .map(|byte| *byte ^ key)
            .collect::<Vec<u8>>(),
        None => bytes[..nul_byte_start + 1].to_vec(),
    };
    let decoded = base64::decode_config(&xor, base64::URL_SAFE)?;
    let mut decompressed = Vec::with_capacity(decoded.len() + decoded.len() / 2);
    flate2::read::GzDecoder::new(&*decoded).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decode_inner_level(bytes: &[u8]) -> Result<Vec<GDLevelObject>, bevy::asset::Error> {
    let mut objects = Vec::new();
    for object_string in bytes.split(|byte| *byte == b';') {
        let mut object = GDLevelObject {
            id: 0,
            x: 0.0,
            y: 0.0,
            flip_x: false,
            flip_y: false,
            rot: 0.0,
            scale: 0.0,
        };
        let mut iterator = object_string.split(|byte| *byte == b',');
        while let Some(property_id) = iterator.next() {
            let property_value = match iterator.next() {
                Some(value) => value,
                None => break,
            };
            match property_id {
                b"1" => object.id = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"2" => {
                    object.x = String::from_utf8_lossy(property_value)
                        .parse::<f32>()
                        .unwrap()
                        * 4.0
                }
                b"3" => {
                    object.y = String::from_utf8_lossy(property_value)
                        .parse::<f32>()
                        .unwrap()
                        * 4.0
                }
                b"4" => object.flip_x = u8_to_bool(property_value),
                b"5" => object.flip_y = u8_to_bool(property_value),
                b"6" => object.rot = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"32" => object.scale = String::from_utf8_lossy(property_value).parse().unwrap(),
                _ => {}
            }
        }
        if object.scale == 0. {
            object.scale = 1.
        }
        if object.id == 0 {
            continue;
        } else {
            objects.push(object)
        }
    }
    Ok(objects)
}

fn u8_to_bool(byte: &[u8]) -> bool {
    match byte {
        b"1" => true,
        _ => false,
    }
}

const FIX_PATTERN: &[&str; 8] = &["<d />", "d>", "k>", "r>", "i>", "s>", "<t", "<f"];
const FIX_REPLACE: &[&str; 8] = &[
    "<dict/>", "dict>", "key>", "real>", "integer>", "string>", "<true", "<false",
];
