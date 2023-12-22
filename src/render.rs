use bevy::app::{PluginGroup, PluginGroupBuilder};

// use crate::render::level::LevelRenderPlugin;
use crate::render::remove_srgb::RemoveSrgbPlugin;

// mod level;
mod remove_srgb;

pub(crate) struct RenderPlugins;

impl PluginGroup for RenderPlugins {
    fn build(self) -> PluginGroupBuilder {
        let mut group = PluginGroupBuilder::start::<Self>();

        group = group.add(RemoveSrgbPlugin);

        group
    }
}
