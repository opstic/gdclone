use aho_corasick::AhoCorasick;
use base64;
use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::BoxedFuture;
use flate2;
use plist;
use plist::{Dictionary, Value};

use serde::Deserialize;
use std::io::Read;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
pub struct GDLevel {
    a: Dictionary,
}

#[derive(Debug, Deserialize)]
struct GDLevelPlist {
    #[serde(rename = "LLM_01")]
    llm_01: String,
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
            let FIX_PATTERN = &[
                "<d />", "d>", "k>", "r>", "i>", "s>", "<t", "<f", "kI7", "kI6", "kI5", "kI4",
                "kI3", "kI2", "kI1", "k4",
            ];
            let FIX_REPLACE = &[
                "<dict/>",
                "dict>",
                "key>",
                "real>",
                "integer>",
                "string>",
                "<true",
                "<false",
                "editorLayer",
                "editorRecentPages",
                "editorBuildTabCategory",
                "editorBuildTabPage",
                "editorCameraZoom",
                "editorCameraX",
                "editorCameraY",
                "levelData",
            ];
            info!("Loading a level");
            let xor: Vec<u8> = bytes.iter().map(|byte| *byte ^ 11).collect();
            let nul_byte_start = xor.iter().rposition(|byte| *byte != 0_u8).unwrap();
            let decoded = base64::decode_config(&xor[0..nul_byte_start+1], base64::URL_SAFE)?;
            let mut fixed = Vec::with_capacity(decoded.len() + decoded.len() / 2);
            let decompressor = flate2::read::GzDecoder::new(&*decoded);
            AhoCorasick::new(FIX_PATTERN).stream_replace_all(decompressor, &mut fixed, FIX_REPLACE)?;
            info!("most done");
            let plist: Dictionary = plist::from_bytes(&fixed)?;
            load_context.set_default_asset(LoadedAsset::new(GDLevel { a: plist }));
            info!("Finished");
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["dat"]
    }
}
