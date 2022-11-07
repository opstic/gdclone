use aho_corasick::AhoCorasick;
use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
#[cfg(not(target_arch = "wasm32"))]
use bevy::tasks::AsyncComputeTaskPool;
use bevy::utils::{BoxedFuture, HashMap};
use plist::Dictionary;
use serde::Deserialize;
use std::io::Read;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "1303d57b-af74-4318-ac9b-5d9e5519bcf1"]
pub struct GDSaveFile {
    pub(crate) levels: Vec<GDLevel>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GDLevel {
    pub(crate) id: Option<u64>,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) start_object: GDStartObject,
    pub(crate) inner_level: Vec<GDLevelObject>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDStartObject {
    pub(crate) colors: HashMap<u128, GDColorChannel>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDLevelObject {
    pub(crate) id: u16,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) rot: f32,
    pub(crate) main_color: u128,
    pub(crate) second_color: u128,
    pub(crate) z_layer: i8,
    pub(crate) z_order: i16,
    pub(crate) scale: f32,
    pub(crate) groups: Vec<u128>,
    pub(crate) other: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDColorChannel {
    pub(crate) index: u128,
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) opacity: f32,
    pub(crate) blending: bool,
}

impl Default for GDStartObject {
    fn default() -> Self {
        GDStartObject {
            colors: HashMap::new(),
        }
    }
}

impl Default for GDLevelObject {
    fn default() -> Self {
        GDLevelObject {
            id: 0,
            x: 0.0,
            y: 0.0,
            flip_x: false,
            flip_y: false,
            rot: 0.0,
            main_color: 0,
            second_color: 0,
            z_layer: 1,
            z_order: 1,
            scale: 1.0,
            groups: Vec::new(),
            other: HashMap::new(),
        }
    }
}

impl Default for GDColorChannel {
    fn default() -> Self {
        GDColorChannel {
            index: 0,
            r: 0,
            g: 0,
            b: 0,
            opacity: 1.,
            blending: false,
        }
    }
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
            let plist_levels = plist.get("LLM_01").unwrap().as_dictionary().unwrap();
            let mut levels: Vec<GDLevel> = Vec::with_capacity(plist_levels.len());

            // TODO: Also use multithreading on wasm once taskpools work on there
            if plist_levels.len() <= 2 || cfg!(target_arch = "wasm32") {
                for (key_name, key) in plist_levels {
                    if key_name != "_isArr" {
                        levels.push(load_level(key.as_dictionary().unwrap()).await?);
                    }
                }
            } else {
                info!("Multithreaded");
                #[cfg(not(target_arch = "wasm32"))]
                AsyncComputeTaskPool::get()
                    .scope(|scope| {
                        plist_levels.into_iter().for_each(|(key_name, key)| {
                            if key_name != "_isArr" {
                                scope.spawn(async move {
                                    load_level(key.as_dictionary().unwrap()).await
                                });
                            }
                        });
                    })
                    .into_iter()
                    .filter_map(|res| {
                        if let Err(err) = res.as_ref() {
                            warn!("Error loading level: {}", err);
                        }
                        res.ok()
                    })
                    .for_each(|level| {
                        levels.push(level);
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

async fn load_level(level_data: &Dictionary) -> Result<GDLevel, bevy::asset::Error> {
    let level_id = level_data
        .get("k1")
        .map(|id| id.as_unsigned_integer().unwrap());
    let level_description = level_data
        .get("k3")
        .map(|description| description.as_string().unwrap().to_string());
    let (level_start, level_inner) = if let Some(inner) = level_data.get("k4") {
        decode_inner_level(&decrypt(inner.as_string().unwrap().as_bytes(), None)?)?
    } else {
        (GDStartObject::default(), Vec::new())
    };
    Ok(GDLevel {
        id: level_id,
        name: level_data
            .get("k2")
            .unwrap()
            .as_string()
            .unwrap()
            .to_string(),
        description: level_description,
        start_object: level_start,
        inner_level: level_inner,
    })
}

fn decrypt(bytes: &[u8], key: Option<u8>) -> Result<Vec<u8>, bevy::asset::Error> {
    let mut xor = Vec::with_capacity(bytes.len());
    let nul_byte_start = bytes
        .iter()
        .rposition(|byte| *byte != 11_u8)
        .unwrap_or(bytes.len() - 1);
    xor.extend(match key {
        Some(key) => bytes[..nul_byte_start + 1]
            .iter()
            .map(|byte| *byte ^ key)
            .collect::<Vec<u8>>(),
        None => bytes[..nul_byte_start + 1].to_vec(),
    });
    let mut decoded = Vec::new();
    base64::decode_engine_vec(xor, &mut decoded, &BASE64_URL_SAFE)?;
    let mut decompressed = Vec::with_capacity(decoded.len() + decoded.len() / 2);
    flate2::read::GzDecoder::new(&*decoded).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decode_inner_level(bytes: &[u8]) -> Result<(GDStartObject, Vec<GDLevelObject>), bevy::asset::Error> {
    let mut objects = Vec::with_capacity(bytes.len() / 100);
    let mut start_object = GDStartObject::default();
    for object_string in bytes.split(|byte| *byte == b';') {
        let mut object = GDLevelObject::default();
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
                b"21" => object.main_color = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"22" => object.second_color = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"24" => object.z_layer = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"25" => object.z_order = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"32" => object.scale = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"57" => object.groups = parse_integer_array(property_value).unwrap(),
                _ => { object.other.insert(String::from_utf8_lossy(property_id).to_string(), String::from_utf8_lossy(property_value).to_string()); }
            }
        }
        if object.id == 0 {
            if object.other.contains_key("kA2") {
                start_object = parse_start_object(object).unwrap();
                continue;
            } else {
                continue;
            }
        } else {
            objects.push(object)
        }
    }
    Ok((start_object, objects))
}

fn parse_start_object(object: GDLevelObject) -> Result<GDStartObject, bevy::asset::Error> {
    let mut start = GDStartObject::default();
    for (property_id, property_value) in object.other {
        match property_id.as_str() {
            "kS38" => start.colors = parse_color_string(property_value.as_bytes()).unwrap(),
            _ => {}
        }
    }
    Ok(start)
}

fn parse_color_string(bytes: &[u8]) -> Result<HashMap<u128, GDColorChannel>, bevy::asset::Error> {
    let mut colors = HashMap::new();
    for color_string in bytes.split(|byte| *byte == b'|') {
        let mut color = GDColorChannel::default();
        let mut iterator = color_string.split(|byte| *byte == b'_');
        while let Some(property_id) = iterator.next() {
            let property_value = match iterator.next() {
                Some(value) => value,
                None => break,
            };
            match property_id {
                b"1" => color.r = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"2" => color.g = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"3" => color.b = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"5" => color.blending = u8_to_bool(property_value),
                b"6" => color.index = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"7" => color.opacity = String::from_utf8_lossy(property_value).parse().unwrap(),
                _ => {}
            }
        }
        colors.insert(color.index, color);
    }
    Ok(colors)
}

fn parse_integer_array(bytes: &[u8]) -> Result<Vec<u128>, bevy::asset::Error> {
    let mut array = Vec::new();
    array.extend(bytes.split(|byte| *byte == b'.').into_iter().map(|b| String::from_utf8_lossy(b).parse::<u128>().unwrap()));
    Ok(array)
}

fn u8_to_bool(byte: &[u8]) -> bool {
    matches!(byte, b"1")
}

const FIX_PATTERN: &[&str; 8] = &["<d />", "d>", "k>", "r>", "i>", "s>", "<t", "<f"];
const FIX_REPLACE: &[&str; 8] = &[
    "<dict/>", "dict>", "key>", "real>", "integer>", "string>", "<true", "<false",
];

const BASE64_URL_SAFE: base64::engine::fast_portable::FastPortable =
    base64::engine::fast_portable::FastPortable::from(
        &base64::alphabet::URL_SAFE,
        base64::engine::fast_portable::PAD,
    );
