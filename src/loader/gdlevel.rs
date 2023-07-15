use std::time::Instant;

use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::{TypePath, TypeUuid};
use bevy::utils::BoxedFuture;
use plist::Dictionary;
use serde::Deserialize;

use crate::level::Level;
use crate::utils::{decompress, decrypt};

#[derive(Debug, Deserialize, TypeUuid, TypePath)]
#[uuid = "1303d57b-af74-4318-ac9b-5d9e5519bcf1"]
pub(crate) struct SaveFile {
    pub(crate) levels: Vec<Level>,
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
            let start = Instant::now();
            let mut levels: Vec<Level> = Vec::new();
            if let Ok(mut decompressed) = match decrypt(bytes, Some(11_u8)) {
                Ok(decrypted) => decompress(&decrypted),
                Err(e) => Err(e),
            } {
                let mut cursor = 0;
                loop {
                    match std::str::from_utf8(&decompressed[cursor..]) {
                        Ok(_) => break,
                        Err(error) => {
                            cursor += error.valid_up_to();
                            if let Some(len) = error.error_len() {
                                decompressed.splice(cursor..cursor + len, [b'\0']);
                            }
                        }
                    }
                }
                let parsed_save: Dictionary =
                    plist::from_bytes::<Dictionary>(&decompressed).unwrap();
                levels.reserve(parsed_save.len() - 1);
                for (key_name, key) in parsed_save.get("LLM_01").unwrap().as_dictionary().unwrap() {
                    if key_name == "_isArr" {
                        continue;
                    }
                    match plist::from_value::<Level>(key.clone()) {
                        Ok(l) => {
                            levels.push(l);
                        }
                        Err(e) => {
                            error!("Cannot load level {}, error: {}", key_name, e);
                        }
                    }
                }
                info!("Save loading done");
                info!("Took {:?}", start.elapsed());
            } else {
                warn!("Corrupted or empty save file");
            }
            load_context.set_default_asset(LoadedAsset::new(SaveFile { levels }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["dat"]
    }
}
