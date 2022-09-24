use bevy::asset::{AssetLoader, BoxedFuture, LoadContext, LoadedAsset};
use bevy::utils::HashMap;
use bevy::reflect::TypeUuid;
use bevy::log::info;
use serde::Deserialize;
use serde_json::value::Value;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "fa9eacab-5f47-4ed5-84a7-6018c950b39d"]
pub struct ObjectMapping {
    pub(crate) mapping: HashMap<u16, String>
}

#[derive(Default)]
pub struct ObjectMappingLoader;

impl AssetLoader for ObjectMappingLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let mappings: Value = serde_json::from_slice(bytes)?;
            let mut object_mapping = HashMap::new();
            for mapping in mappings.as_array().unwrap() {
                let map = mapping.as_object().unwrap();
                object_mapping.insert(map.get("id").unwrap().as_u64().unwrap() as u16, map.get("sprite").unwrap().as_str().unwrap().to_string());
            }
            info!("{:?}", object_mapping);
            load_context.set_default_asset(LoadedAsset::new(ObjectMapping {
                mapping: object_mapping
            }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["mapping"]
    }
}