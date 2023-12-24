use bevy::app::{PluginGroup, PluginGroupBuilder};

use crate::render::object::ObjectRenderPlugin;
use crate::render::remove_srgb::RemoveSrgbPlugin;

mod object;
mod remove_srgb;

pub(crate) struct RenderPlugins;

impl PluginGroup for RenderPlugins {
    fn build(self) -> PluginGroupBuilder {
        let mut group = PluginGroupBuilder::start::<Self>();

        group = group.add(ObjectRenderPlugin).add(RemoveSrgbPlugin);

        group
    }
}
