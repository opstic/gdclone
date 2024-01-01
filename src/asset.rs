use bevy::app::{App, Plugin, PreUpdate, Startup};
use bevy::asset::{AssetApp, AssetServer, Handle};
use bevy::prelude::{Commands, Res, Resource};
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
            .add_systems(Startup, load_assets)
            .add_systems(PreUpdate, cocos2d_atlas::move_frames_to_resource);
    }
}

#[derive(Resource)]
pub(crate) struct TestAssets {
    pub(crate) assets: Vec<Handle<Cocos2dAtlas>>,
}

fn load_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let a = TestAssets {
        assets: vec![
            asset_server.load("GJ_GameSheet-uhd.plist"),
            asset_server.load("GJ_GameSheet02-uhd.plist"),
            asset_server.load("GJ_GameSheet03-uhd.plist"),
            asset_server.load("GJ_GameSheet04-uhd.plist"),
            asset_server.load("GJ_GameSheetGlow-uhd.plist"),
        ],
    };

    commands.insert_resource(a);
}
