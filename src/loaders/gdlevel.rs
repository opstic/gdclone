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
pub(crate) struct GDSaveFile {
    pub(crate) levels: Vec<GDLevel>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDLevel {
    pub(crate) id: Option<u64>,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) start_object: GDStartObject,
    pub(crate) inner_level: Vec<GDLevelObject>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDStartObject {
    pub(crate) colors: HashMap<u64, GDColorChannel>,
}

#[derive(Debug, Deserialize, Clone, Reflect)]
pub(crate) struct GDLevelObject {
    pub(crate) id: u16,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) rot: f32,
    pub(crate) main_color: u64,
    pub(crate) second_color: u64,
    pub(crate) z_layer: i8,
    pub(crate) z_order: i16,
    pub(crate) scale: f32,
    pub(crate) main_hsv_enabled: bool,
    pub(crate) second_hsv_enabled: bool,
    pub(crate) main_hsv: GDHSV,
    pub(crate) second_hsv: GDHSV,
    pub(crate) groups: Vec<u64>,
    pub(crate) other: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone, Reflect)]
pub(crate) struct GDHSV {
    pub(crate) h: f32,
    pub(crate) s: f32,
    pub(crate) v: f32,
    pub(crate) checked_s: i64,
    pub(crate) checked_v: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDBaseColor {
    pub(crate) index: u64,
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
    pub(crate) opacity: f32,
    pub(crate) original_r: u8,
    pub(crate) original_g: u8,
    pub(crate) original_b: u8,
    pub(crate) original_opacity: f32,
    pub(crate) blending: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct GDCopyColor {
    pub(crate) index: u64,
    pub(crate) copied_index: u64,
    pub(crate) copy_opacity: bool,
    pub(crate) opacity: f32,
    pub(crate) blending: bool,
    pub(crate) hsv: GDHSV,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) enum GDColorChannel {
    BaseColor(GDBaseColor),
    CopyColor(GDCopyColor),
}

impl Default for GDColorChannel {
    fn default() -> Self {
        GDColorChannel::BaseColor(GDBaseColor::default())
    }
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
            main_hsv_enabled: false,
            second_hsv_enabled: false,
            main_hsv: GDHSV::default(),
            second_hsv: GDHSV::default(),
            groups: Vec::new(),
            other: HashMap::new(),
        }
    }
}

impl Default for GDHSV {
    fn default() -> Self {
        GDHSV {
            h: 0.,
            s: 1.,
            v: 1.,
            checked_s: 0,
            checked_v: 0,
        }
    }
}

impl Default for GDBaseColor {
    fn default() -> Self {
        GDBaseColor {
            index: 0,
            r: 255,
            g: 255,
            b: 255,
            opacity: 1.,
            original_r: 255,
            original_g: 255,
            original_b: 255,
            original_opacity: 1.,
            blending: false,
        }
    }
}

impl Default for GDCopyColor {
    fn default() -> Self {
        GDCopyColor {
            index: 0,
            copied_index: 0,
            copy_opacity: false,
            opacity: 1.,
            blending: false,
            hsv: GDHSV::default(),
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

fn decode_inner_level(
    bytes: &[u8],
) -> Result<(GDStartObject, Vec<GDLevelObject>), bevy::asset::Error> {
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
                b"21" => {
                    object.main_color = String::from_utf8_lossy(property_value).parse().unwrap()
                }
                b"22" => {
                    object.second_color = String::from_utf8_lossy(property_value).parse().unwrap()
                }
                b"24" => object.z_layer = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"25" => object.z_order = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"32" => object.scale = String::from_utf8_lossy(property_value).parse().unwrap(),
                b"57" => object.groups = parse_integer_array(property_value).unwrap(),
                _ => {
                    object.other.insert(
                        String::from_utf8_lossy(property_id).to_string(),
                        String::from_utf8_lossy(property_value).to_string(),
                    );
                }
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

fn parse_color_string(bytes: &[u8]) -> Result<HashMap<u64, GDColorChannel>, bevy::asset::Error> {
    let mut colors = HashMap::new();
    for color_string in bytes.split(|byte| *byte == b'|') {
        let mut properties = HashMap::new();
        let mut iterator = color_string.split(|byte| *byte == b'_');
        while let Some(property_id) = iterator.next() {
            let property_value = match iterator.next() {
                Some(value) => value,
                None => break,
            };
            properties.insert(
                String::from_utf8_lossy(property_id),
                String::from_utf8_lossy(property_value),
            );
        }
        let mut index = 0;
        if let Some(got_index) = properties.get("6") {
            index = got_index.parse().unwrap();
        } else {
            continue;
        }
        let mut color: GDColorChannel;
        if properties.contains_key("9") {
            let mut temp_color = GDCopyColor::default();
            temp_color.index = index;
            temp_color.copied_index = if let Some(value) = properties.get("9") {
                value.parse().unwrap()
            } else {
                0
            };
            temp_color.copy_opacity = if let Some(value) = properties.get("17") {
                u8_to_bool(value.as_bytes())
            } else {
                false
            };
            temp_color.opacity = if let Some(value) = properties.get("7") {
                value.parse().unwrap()
            } else {
                1.
            };
            temp_color.blending = if let Some(value) = properties.get("5") {
                u8_to_bool(value.as_bytes())
            } else {
                false
            };
            temp_color.hsv = if let Some(value) = properties.get("10") {
                parse_hsv_string(value.as_bytes()).unwrap()
            } else {
                GDHSV::default()
            };
            color = GDColorChannel::CopyColor(temp_color);
        } else {
            let mut temp_color = GDBaseColor::default();
            temp_color.index = index;
            temp_color.r = if let Some(value) = properties.get("1") {
                value.parse().unwrap()
            } else {
                255
            };
            temp_color.original_r = temp_color.r;
            temp_color.g = if let Some(value) = properties.get("2") {
                value.parse().unwrap()
            } else {
                255
            };
            temp_color.original_g = temp_color.g;
            temp_color.b = if let Some(value) = properties.get("3") {
                value.parse().unwrap()
            } else {
                255
            };
            temp_color.original_b = temp_color.b;
            temp_color.opacity = if let Some(value) = properties.get("7") {
                value.parse().unwrap()
            } else {
                1.
            };
            temp_color.original_opacity = temp_color.opacity;
            temp_color.blending = if let Some(value) = properties.get("5") {
                u8_to_bool(value.as_bytes())
            } else {
                false
            };
            color = GDColorChannel::BaseColor(temp_color);
        }
        colors.insert(index, color);
    }
    Ok(colors)
}

fn parse_hsv_string(bytes: &[u8]) -> Result<GDHSV, bevy::asset::Error> {
    let mut hsv = GDHSV::default();
    for (i, bytes) in bytes.split(|byte| *byte == b'a').enumerate() {
        match i {
            0 => hsv.h = String::from_utf8_lossy(bytes).parse().unwrap(),
            1 => hsv.s = String::from_utf8_lossy(bytes).parse().unwrap(),
            2 => hsv.v = String::from_utf8_lossy(bytes).parse().unwrap(),
            3 => hsv.checked_s = String::from_utf8_lossy(bytes).parse().unwrap(),
            4 => hsv.checked_v = String::from_utf8_lossy(bytes).parse().unwrap(),
            _ => {}
        }
    }
    Ok(hsv)
}

fn parse_integer_array(bytes: &[u8]) -> Result<Vec<u64>, bevy::asset::Error> {
    let mut array = Vec::new();
    array.extend(
        bytes
            .split(|byte| *byte == b'.')
            .into_iter()
            .map(|b| String::from_utf8_lossy(b).parse::<u64>().unwrap()),
    );
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
