use bevy::prelude::Component;

#[derive(Clone, Component, Default)]
pub(crate) struct Object {
    pub(crate) id: u16,
    frame_name: String,
    flip_x: bool,
    flip_y: bool,
    pub(crate) z_layer: i32,
}
