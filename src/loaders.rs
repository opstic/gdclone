use bevy::prelude::*;

pub(crate) mod cocos2d_atlas;
pub(crate) mod gdlevel;

use crate::loaders::cocos2d_atlas::Cocos2dAtlasSprite;
use cocos2d_atlas::{Cocos2dAtlas, Cocos2dAtlasLoader, Cocos2dFrames};
use gdlevel::{GDSaveLoader, SaveFile};

pub(crate) struct AssetLoaderPlugin;

impl Plugin for AssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<SaveFile>()
            .add_asset::<Cocos2dAtlas>()
            .init_asset_loader::<GDSaveLoader>()
            .init_asset_loader::<Cocos2dAtlasLoader>()
            .init_resource::<Cocos2dFrames>()
            .register_type::<Cocos2dAtlasSprite>()
            .add_system(cocos2d_atlas::add_frames_to_resource);
    }
}
