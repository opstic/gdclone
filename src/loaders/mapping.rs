use bevy::asset::{AssetLoader, BoxedFuture, LoadContext, LoadedAsset};
use bevy::reflect::TypeUuid;
use bevy::utils::HashMap;
use serde::Deserialize;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "fa9eacab-5f47-4ed5-84a7-6018c950b39d"]
pub struct Mapping(pub(crate) HashMap<u64, ObjectMetadata>);

#[derive(Debug, Deserialize)]
pub(crate) struct ObjectMetadata {
    #[serde(default)]
    pub(crate) texture_name: String,
    #[serde(default)]
    pub(crate) default_z_layer: i8,
    #[serde(default)]
    pub(crate) default_z_order: i16,
}

#[derive(Default)]
pub struct MappingLoader;

impl AssetLoader for MappingLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let mapping: HashMap<u64, ObjectMetadata> = serde_json::from_slice(bytes).unwrap();
            load_context.set_default_asset(LoadedAsset::new(Mapping(mapping)));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json.mapping", "mapping"]
    }
}
