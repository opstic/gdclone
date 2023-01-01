use bevy::asset::{AssetLoader, BoxedFuture, LoadContext, LoadedAsset};
use bevy::reflect::TypeUuid;
use bevy::utils::HashMap;
use serde::Deserialize;

#[derive(Debug, Deserialize, TypeUuid)]
#[uuid = "fa9eacab-5f47-4ed5-84a7-6018c950b39d"]
pub struct Mapping {
    pub(crate) mapping: HashMap<u64, String>,
}

#[derive(Deserialize)]
pub(crate) struct Map {
    pub(crate) id: u64,
    pub(crate) value: String,
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
            let maps: Vec<Map> = serde_json::from_slice(bytes)?;
            let mut mapping = HashMap::new();
            mapping.extend(maps.iter().map(|m| (m.id, m.value.clone())));
            load_context.set_default_asset(LoadedAsset::new(Mapping { mapping }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json.mapping", "mapping"]
    }
}
