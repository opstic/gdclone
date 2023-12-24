use bevy::math::{Vec2, Vec3};
use bevy::prelude::Component;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) enum ObjectColorType {
    Base,
    Detail,
    Black,
    #[default]
    None,
}

struct ObjectDefaultData {
    texture: &'static str,
    default_z_layer: i8,
    default_z_order: i16,
    default_base_color_channel: u64,
    default_detail_color_channel: u64,
    color_type: ObjectColorType,
    swap_base_detail: bool,
    opacity: f32,
    children: &'static [ObjectChild],
}

impl Default for ObjectDefaultData {
    fn default() -> Self {
        ObjectDefaultData {
            texture: "emptyFrame.png",
            default_z_layer: 0,
            default_z_order: 0,
            default_base_color_channel: u64::MAX,
            default_detail_color_channel: u64::MAX,
            color_type: ObjectColorType::None,
            swap_base_detail: false,
            opacity: 1.,
            children: &[],
        }
    }
}

struct ObjectChild {
    texture: &'static str,
    offset: Vec3,
    rotation: f32,
    anchor: Vec2,
    scale: Vec2,
    flip_x: bool,
    flip_y: bool,
    color_type: ObjectColorType,
    opacity: f32,
    children: &'static [ObjectChild],
}

impl Default for ObjectChild {
    fn default() -> Self {
        ObjectChild {
            texture: "emptyFrame.png",
            offset: Vec3::ZERO,
            rotation: 0.,
            anchor: Vec2::ZERO,
            scale: Vec2::ONE,
            flip_x: false,
            flip_y: false,
            color_type: ObjectColorType::None,
            opacity: 1.,
            children: &[],
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/generated_object.rs"));

#[derive(Clone, Component, Default)]
pub(crate) struct Object {
    pub(crate) id: u16,
    frame_name: String,
    flip_x: bool,
    flip_y: bool,
    pub(crate) z_layer: i32,
}
