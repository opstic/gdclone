use aho_corasick::AhoCorasick;
use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::BoxedFuture;
use plist::Dictionary;
use serde::Deserialize;
use std::io::Read;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "1303d57b-af74-4318-ac9b-5d9e5519bcf1"]
pub struct GDLevel {
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
pub struct GDLevelLoader;

impl AssetLoader for GDLevelLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            info!("Loading a level");
            let decrypted = decrypt(bytes, Some(11_u8))?;
            let fixed = AhoCorasick::new(FIX_PATTERN).replace_all_bytes(&decrypted, FIX_REPLACE);
            let plist: Dictionary = plist::from_bytes(&fixed)?;
            let inner_level_encoded = plist
                .get("LLM_01")
                .unwrap()
                .as_dictionary()
                .unwrap()
                .get("k_0")
                .unwrap()
                .as_dictionary()
                .unwrap()
                .get("k4")
                .unwrap()
                .as_string()
                .unwrap();
            let inner_level_string = decrypt(inner_level_encoded.as_bytes(), None)?;
            let inner_level = decode_inner_level(&inner_level_string)?;
            // info!("{:?}", inner_level);
            load_context.set_default_asset(LoadedAsset::new(GDLevel { inner_level }));
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
