use bevy::prelude::*;

pub(crate) mod gdlevel;

use gdlevel::{GDLevel, GDLevelLoader};

pub(crate) struct AssetLoaderPlugin;

impl Plugin for AssetLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<GDLevel>()
            .init_asset_loader::<GDLevelLoader>();
    }
}
