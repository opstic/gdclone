use bevy::app::{App, Plugin, PreUpdate};
use bevy::asset::{AssetApp, Handle};
use bevy::prelude::Resource;
use bevy::render::render_asset::RenderAssetPlugin;

use crate::asset::cocos2d_atlas::Cocos2dFrames;
use crate::asset::{
    cocos2d_atlas::{Cocos2dAtlas, Cocos2dAtlasLoader},
    compressed_image::CompressedImage,
};

pub(crate) mod cocos2d_atlas;
pub(crate) mod compressed_image;

pub(crate) struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenderAssetPlugin::<CompressedImage>::default())
            .init_asset::<CompressedImage>()
            .init_asset::<Cocos2dAtlas>()
            .init_asset_loader::<Cocos2dAtlasLoader>()
            .init_resource::<Cocos2dFrames>()
            .add_systems(PreUpdate, cocos2d_atlas::move_frames_to_resource);
    }
}

#[derive(Resource)]
pub(crate) struct GlobalAssets {
    pub(crate) assets: Vec<Handle<Cocos2dAtlas>>,
}
