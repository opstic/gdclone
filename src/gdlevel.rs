use aho_corasick::AhoCorasick;
use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::BoxedFuture;
use plist::Dictionary;
use serde::Deserialize;
use std::io::Read;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
pub struct GDLevel {
    a: Dictionary,
}

pub struct GDLevelObject {
    id: u16,
    x: f32,
    y: f32,
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
            info!("{:#?}", String::from_utf8_lossy(&inner_level_string[..500]));
            load_context.set_default_asset(LoadedAsset::new(GDLevel { a: plist }));
            info!("Finished");
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
            .map(|byte| *byte ^ 11)
            .collect::<Vec<u8>>(),
        None => bytes[..nul_byte_start + 1].to_vec(),
    };
    let decoded = base64::decode_config(&xor, base64::URL_SAFE)?;
    let mut decompressed = Vec::with_capacity(decoded.len() + decoded.len() / 2);
    flate2::read::GzDecoder::new(&*decoded).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

const FIX_PATTERN: &[&str; 8] = &["<d />", "d>", "k>", "r>", "i>", "s>", "<t", "<f"];
const FIX_REPLACE: &[&str; 8] = &[
    "<dict/>", "dict>", "key>", "real>", "integer>", "string>", "<true", "<false",
];
