use bevy::prelude::*;

pub(crate) mod cocos2d_atlas;
pub(crate) mod mapping;
// mod render;
pub(crate) mod gdlevel;

use cocos2d_atlas::{Cocos2dAtlas, Cocos2dAtlasLoader};
use gdlevel::{GDSaveFile, GDSaveLoader};
use mapping::{ObjectMapping, ObjectMappingLoader};

pub(crate) struct AssetLoaderPlugin;

impl Plugin for AssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<GDSaveFile>()
            .add_asset::<ObjectMapping>()
            .add_asset::<Cocos2dAtlas>()
            .init_asset_loader::<GDSaveLoader>()
            .init_asset_loader::<ObjectMappingLoader>()
            .init_asset_loader::<Cocos2dAtlasLoader>();
    }
}
