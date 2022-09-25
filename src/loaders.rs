use bevy::prelude::*;

pub(crate) mod gdlevel;
pub(crate) mod mapping;
pub(crate) mod texture_packer;

use gdlevel::{GDSaveFile, GDSaveLoader};
use mapping::{ObjectMapping, ObjectMappingLoader};
use texture_packer::{TexturePackerAtlas, TexturePackerAtlasLoader};

pub(crate) struct AssetLoaderPlugin;

impl Plugin for AssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<GDSaveFile>()
            .add_asset::<ObjectMapping>()
            .add_asset::<TexturePackerAtlas>()
            .init_asset_loader::<GDSaveLoader>()
            .init_asset_loader::<ObjectMappingLoader>()
            .init_asset_loader::<TexturePackerAtlasLoader>();
    }
}
