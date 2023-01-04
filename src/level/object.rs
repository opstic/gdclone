use crate::level::color::{ColorChannel, HSV};
use crate::level::easing::Easing;
use crate::level::trigger::TriggerDuration;
use bevy::prelude::Reflect;
use bevy::utils::HashMap;
use serde::Deserialize;

// THANK YOU https://gdprogra.me/
// also can't figure out a better way to do this
#[derive(Debug, Default, Deserialize, Reflect)]
pub(crate) struct Object {
    #[serde(rename = "1")]
    id: Option<u16>,
    #[serde(rename = "2")]
    x: Option<f32>,
    #[serde(rename = "3")]
    y: Option<f32>,
    #[serde(rename = "4")]
    flip_x: Option<bool>,
    #[serde(rename = "5")]
    flip_y: Option<bool>,
    #[serde(rename = "6")]
    rot: Option<f32>,
    #[serde(rename = "7")]
    trigger_r: Option<u8>,
    #[serde(rename = "8")]
    trigger_g: Option<u8>,
    #[serde(rename = "9")]
    trigger_b: Option<u8>,
    #[serde(rename = "10")]
    trigger_duration: Option<TriggerDuration>,
    #[serde(rename = "11")]
    trigger_touch: Option<bool>,
    #[serde(rename = "12")]
    coin_id: Option<u8>,
    #[serde(rename = "13")]
    checked: Option<bool>,
    #[serde(rename = "14")]
    trigger_tint_ground: Option<bool>,
    #[serde(rename = "15")]
    trigger_player_color_1: Option<bool>,
    #[serde(rename = "16")]
    trigger_player_color_2: Option<bool>,
    #[serde(rename = "17")]
    trigger_blending: Option<bool>,
    // ??? why is this bool
    #[serde(rename = "20")]
    editor_layer_1: Option<bool>,
    // yay 18,446,744,073,709,551,615 color channels available
    #[serde(rename = "21")]
    main_color: Option<u64>,
    #[serde(rename = "22")]
    second_color: Option<u64>,
    #[serde(rename = "23")]
    trigger_target_color: Option<u64>,
    #[serde(rename = "24")]
    z_layer: Option<i8>,
    #[serde(rename = "25")]
    z_order: Option<i16>,
    #[serde(rename = "28")]
    trigger_offset_x: Option<f32>,
    #[serde(rename = "29")]
    trigger_offset_y: Option<f32>,
    #[serde(rename = "30")]
    trigger_easing: Option<Easing>,
    #[serde(rename = "31")]
    text: Option<String>,
    #[serde(rename = "32")]
    scaling: Option<f32>,
    #[serde(rename = "34")]
    group_parent: Option<bool>,
    #[serde(rename = "35")]
    trigger_opacity: Option<f32>,
    #[serde(rename = "41")]
    main_color_hsv_enabled: Option<bool>,
    #[serde(rename = "42")]
    second_color_hsv_enabled: Option<bool>,
    #[serde(rename = "43")]
    main_color_hsv: Option<HSV>,
    #[serde(rename = "44")]
    second_color_hsv: Option<HSV>,
    #[serde(rename = "45")]
    trigger_fade_in: Option<f32>,
    #[serde(rename = "46")]
    trigger_hold: Option<f32>,
    #[serde(rename = "47")]
    trigger_fade_out: Option<f32>,
    // pulse triggers are not there yet
    // #[serde(rename = "48")]
    // pulse_mode: Option<PulseMode>
    #[serde(rename = "49")]
    trigger_copied_color_hsv: Option<HSV>,
    #[serde(rename = "50")]
    trigger_copied_color: Option<u64>,
    // yay 18,446,744,073,709,551,615 groups avaliable
    #[serde(rename = "51")]
    trigger_group_1: Option<u64>,
    // #[serde(rename = "52")]
    // pulse_mode: Option<PulseTarget>
    #[serde(rename = "54")]
    teleport_offset: Option<f32>,
    #[serde(rename = "55")]
    teleport_ease: Option<bool>,
    #[serde(rename = "56")]
    trigger_activate: Option<bool>,
    #[serde(rename = "57")]
    groups: Option<Vec<u64>>,
    #[serde(rename = "58")]
    trigger_lock_x: Option<bool>,
    #[serde(rename = "59")]
    trigger_lock_y: Option<bool>,
    #[serde(rename = "60")]
    trigger_copy_opacity: Option<bool>,
    #[serde(rename = "61")]
    editor_layer_2: Option<i16>,
    #[serde(rename = "62")]
    trigger_spawn_activated: Option<bool>,
    #[serde(rename = "63")]
    trigger_spawn_delay: Option<f32>,
    #[serde(rename = "64")]
    dont_fade: Option<bool>,
    #[serde(rename = "65")]
    trigger_main_only: Option<bool>,
    #[serde(rename = "66")]
    trigger_second_only: Option<bool>,
    #[serde(rename = "67")]
    dont_enter: Option<bool>,
    #[serde(rename = "68")]
    trigger_degrees: Option<i32>,
    #[serde(rename = "69")]
    trigger_times_360: Option<i32>,
    #[serde(rename = "70")]
    trigger_lock_rotation: Option<bool>,
    #[serde(rename = "71")]
    trigger_group_2: Option<u64>,
    #[serde(rename = "72")]
    trigger_x_mod: Option<f32>,
    #[serde(rename = "73")]
    trigger_y_mod: Option<f32>,
    #[serde(rename = "75")]
    trigger_strength: Option<f32>,
    #[serde(rename = "76")]
    trigger_animation: Option<i8>,
    #[serde(rename = "77")]
    trigger_count: Option<i16>,
    #[serde(rename = "78")]
    trigger_subtract: Option<bool>,
    // pickup also not there
    // #[serde(rename = "79")]
    // trigger_pickup: Option<Pickup>,
    #[serde(rename = "80")]
    item: Option<u16>,
    // start object properties
    #[serde(rename = "kA1")]
    audio: Option<u32>,
    #[serde(rename = "kS38")]
    color_channels: Option<HashMap<u64, ColorChannel>>,
    // TODO: add the rest
}
