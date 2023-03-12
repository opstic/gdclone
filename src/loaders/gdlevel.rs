use crate::level::Level;
use crate::utils::{decompress, decrypt};
use bevy::asset::{AssetLoader, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::BoxedFuture;
use plist::Dictionary;
use serde::Deserialize;

#[derive(Debug, Deserialize, TypeUuid)]
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
            let decrypted = decompress(decrypt(bytes, Some(11_u8)).unwrap().as_slice()).unwrap();
            let parsed_save: Dictionary = plist::from_bytes::<Dictionary>(&decrypted).unwrap();
            let mut levels: Vec<Level> = Vec::with_capacity(parsed_save.len() - 1);
            for (key_name, key) in parsed_save.get("LLM_01").unwrap().as_dictionary().unwrap() {
                if key_name == "_isArr" {
                    continue;
                }
                match plist::from_value::<Level>(key.clone()) {
                    Ok(l) => {
                        levels.push(l);
                    }
                    Err(e) => {
                        println!("{:?}", key);
                        panic!("{:?}", e);
                    }
                }
            }
            load_context.set_default_asset(LoadedAsset::new(SaveFile { levels }));
            info!("Done");
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["dat"]
    }
}
