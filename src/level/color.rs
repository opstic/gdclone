use std::cell::Cell;

use bevy::prelude::{Color, Entity, Local, Query, Res, ResMut, Resource};
use bevy::reflect::Reflect;
use bevy::render::view::VisibleEntities;
use bevy::utils::HashMap;
use serde::Deserialize;
use thread_local::ThreadLocal;

use crate::level::{
    de,
    object::{Object, ObjectColorType},
    Groups,
};
use crate::loader::cocos2d_atlas::Cocos2dAtlasSprite;
use crate::par_iter_many;
use crate::utils::{hsv_to_rgb, lerp_color, rgb_to_hsv, u8_to_bool, PassHashMap};

#[derive(Default, Resource)]
pub(crate) struct ColorChannels(pub(crate) PassHashMap<(ColorChannel, Option<ColorMod>)>);

impl ColorChannels {
    pub(crate) fn get_color(&self, index: &u64) -> (Color, bool) {
        self.get_color_inner(index, &mut PassHashMap::default(), false)
    }

    fn get_color_inner(
        &self,
        index: &u64,
        seen: &mut PassHashMap<usize>,
        no_color_mod: bool,
    ) -> (Color, bool) {
        match self
            .0
            .get(index)
            .unwrap_or(&(ColorChannel::default(), None))
        {
            (ColorChannel::BaseColor(color), color_mod) => {
                let mut final_color = color.color;
                if !no_color_mod {
                    if let Some(color_mod) = color_mod {
                        match color_mod {
                            ColorMod::Color(target_color, progress) => {
                                final_color = lerp_color(&final_color, target_color, progress);
                            }
                            ColorMod::Hsv(target_channel, hsv, progress) => {
                                let target_color = if target_channel == index {
                                    color.color
                                } else {
                                    self.get_color_inner(
                                        target_channel,
                                        &mut PassHashMap::default(),
                                        true,
                                    )
                                    .0
                                };
                                final_color =
                                    lerp_color(&final_color, &hsv.apply(target_color), progress);
                            }
                        }
                    }
                }
                (final_color, color.blending)
            }
            (ColorChannel::CopyColor(color), color_mod) => {
                let check = seen.entry(*index).or_default();
                *check += 1;
                let (original_color, _) = if *check > 3 {
                    (Color::WHITE, false)
                } else {
                    self.get_color_inner(&color.copied_index, seen, no_color_mod)
                };
                let mut transformed_color = color.hsv.apply(original_color);
                if !color.copy_opacity {
                    transformed_color.set_a(color.opacity);
                }
                let mut final_color = transformed_color;
                if !no_color_mod {
                    if let Some(color_mod) = color_mod {
                        match color_mod {
                            ColorMod::Color(target_color, progress) => {
                                final_color = lerp_color(&final_color, target_color, progress);
                            }
                            ColorMod::Hsv(target_channel, hsv, progress) => {
                                let target_color = self
                                    .get_color_inner(
                                        target_channel,
                                        &mut PassHashMap::default(),
                                        true,
                                    )
                                    .0;
                                final_color =
                                    lerp_color(&final_color, &hsv.apply(target_color), progress);
                            }
                        }
                    }
                }
                (final_color, color.blending)
            }
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) enum ColorChannel {
    BaseColor(BaseColor),
    CopyColor(CopyColor),
}

impl ColorChannel {
    pub(crate) fn parse(color_string: &[u8]) -> Result<(u64, ColorChannel), anyhow::Error> {
        let color_data: HashMap<&[u8], &[u8]> = de::from_slice(color_string, b'_')?;
        let color;
        if color_data.contains_key(b"9".as_ref()) {
            let mut temp_color = CopyColor::default();
            if let Some(copied_index) = color_data.get(b"9".as_ref()) {
                temp_color.copied_index = std::str::from_utf8(copied_index)?.parse()?;
            }
            if let Some(copy_opacity) = color_data.get(b"17".as_ref()) {
                temp_color.copy_opacity = u8_to_bool(copy_opacity);
            }
            if let Some(opacity) = color_data.get(b"7".as_ref()) {
                temp_color.opacity = std::str::from_utf8(opacity)?.parse()?;
            }
            if let Some(blending) = color_data.get(b"5".as_ref()) {
                temp_color.blending = u8_to_bool(blending);
            }
            if let Some(hsv) = color_data.get(b"10".as_ref()) {
                temp_color.hsv = HsvMod::parse(hsv)?;
            }
            color = ColorChannel::CopyColor(temp_color);
        } else {
            let mut temp_color = BaseColor::default();
            if let Some(r) = color_data.get(b"1".as_ref()) {
                temp_color
                    .color
                    .set_r(std::str::from_utf8(r)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(g) = color_data.get(b"2".as_ref()) {
                temp_color
                    .color
                    .set_g(std::str::from_utf8(g)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(b) = color_data.get(b"3".as_ref()) {
                temp_color
                    .color
                    .set_b(std::str::from_utf8(b)?.parse::<u8>()? as f32 / u8::MAX as f32);
            }
            if let Some(opacity) = color_data.get(b"7".as_ref()) {
                temp_color
                    .color
                    .set_a(std::str::from_utf8(opacity)?.parse()?);
            }
            if let Some(blending) = color_data.get(b"5".as_ref()) {
                temp_color.blending = u8_to_bool(blending);
            }
            color = ColorChannel::BaseColor(temp_color);
        }
        let index = std::str::from_utf8(
            color_data
                .get(b"6".as_ref())
                .ok_or(anyhow::Error::msg("No index in color???"))?,
        )?
        .parse()?;
        Ok((index, color))
    }
}

#[derive(Clone, Debug, Copy)]
pub(crate) enum ColorMod {
    Color(Color, f32),
    Hsv(u64, HsvMod, f32),
}

impl Default for ColorMod {
    fn default() -> Self {
        ColorMod::Color(Color::WHITE, 1.)
    }
}

pub(crate) fn update_light_bg(mut color_channels: ResMut<ColorChannels>) {
    let (bg_color, _) = color_channels.get_color(&1000);
    let mut bg_hsv = rgb_to_hsv([bg_color.r(), bg_color.g(), bg_color.b()]);
    bg_hsv.1 -= 20.;
    let bg_color = hsv_to_rgb(bg_hsv);
    let bg_color = Color::rgb(bg_color[0], bg_color[1], bg_color[2]);
    let (player_color, _) = color_channels.get_color(&1005);
    color_channels.0.insert(
        1007,
        (
            ColorChannel::BaseColor(BaseColor {
                color: lerp_color(&player_color, &bg_color, &(bg_hsv.2 / 100.)),
                blending: true,
            }),
            None,
        ),
    );
}

pub(crate) fn calculate_object_color(
    mut thread_to_be_removed: Local<ThreadLocal<Cell<Vec<Entity>>>>,
    mut to_be_removed: Local<Vec<Entity>>,
    mut thread_cached_colors: Local<ThreadLocal<Cell<PassHashMap<(Color, bool)>>>>,
    mut object_query: Query<(Entity, &Object, &mut Cocos2dAtlasSprite)>,
    mut visible_entities_query: Query<&mut VisibleEntities>,
    groups: Res<Groups>,
    color_channels: Res<ColorChannels>,
) {
    for mut visible_entities in &mut visible_entities_query {
        par_iter_many::par_iter_many_mut(&mut object_query, &visible_entities.entities)
            .for_each_mut(|(entity, object, mut sprite)| {
                let mut opacity = 1.;
                let mut color_mod = None;
                for group_id in &object.groups {
                    if let Some((group, base_color_mod, detail_color_mod)) = groups.0.get(group_id)
                    {
                        if !group.activated || group.opacity == 0. {
                            let cell = thread_to_be_removed.get_or_default();
                            let mut to_be_removed = cell.take();
                            to_be_removed.push(entity);
                            cell.set(to_be_removed);
                            return;
                        }
                        opacity *= group.opacity;
                        if base_color_mod.is_some() && object.color_type == ObjectColorType::Base {
                            color_mod = *base_color_mod;
                        } else if detail_color_mod.is_some()
                            && object.color_type == ObjectColorType::Detail
                        {
                            color_mod = *detail_color_mod;
                        }
                    }
                }

                let cell = thread_cached_colors.get_or_default();
                let mut cached_colors = cell.take();
                let (mut color, blending) =
                    if let Some(color_results) = cached_colors.get(&object.color_channel) {
                        *color_results
                    } else {
                        let color_results = color_channels.get_color(&object.color_channel);
                        cached_colors.insert(object.color_channel, color_results);
                        color_results
                    };
                if let Some(color_mod) = color_mod {
                    color = match color_mod {
                        ColorMod::Color(target_color, progress) => {
                            lerp_color(&color, &target_color, &progress)
                        }
                        ColorMod::Hsv(target_channel, hsv, progress) => {
                            let (target_color, _) =
                                if let Some(color_results) = cached_colors.get(&target_channel) {
                                    *color_results
                                } else {
                                    let color_results = color_channels.get_color(&target_channel);
                                    cached_colors.insert(target_channel, color_results);
                                    color_results
                                };
                            lerp_color(&color, &hsv.apply(target_color), &progress)
                        }
                    }
                }
                cell.set(cached_colors);

                if let Some(hsv) = &object.hsv {
                    color = hsv.apply(color);
                }
                color.set_a(color.a() * opacity * object.opacity);
                if color.a() == 0. {
                    let cell = thread_to_be_removed.get_or_default();
                    let mut to_be_removed = cell.take();
                    to_be_removed.push(entity);
                    cell.set(to_be_removed);
                    return;
                }
                if blending {
                    if object.color_type == ObjectColorType::Black {
                        let cell = thread_to_be_removed.get_or_default();
                        let mut to_be_removed = cell.take();
                        to_be_removed.push(entity);
                        cell.set(to_be_removed);
                        return;
                    }
                    let transformed_opacity = (0.175656971639325_f64
                        * 7.06033051530761_f64.powf(color.a() as f64)
                        - 0.213355914301931_f64)
                        .clamp(0., 1.) as f32;
                    color.set_a(transformed_opacity);
                }
                if object.color_type == ObjectColorType::Black {
                    color = Color::rgba(0., 0., 0., color.a());
                }
                sprite.color = color;
                sprite.blending = blending;
            });

        to_be_removed.clear();
        for cell in thread_to_be_removed.iter_mut() {
            to_be_removed.append(cell.get_mut());
        }
        radsort::sort_by_key(&mut to_be_removed, |entity| entity.index());
        visible_entities
            .entities
            .retain(|entity| to_be_removed.binary_search(entity).is_err())
    }

    for cell in &mut thread_cached_colors {
        cell.get_mut().clear();
    }
}

#[derive(Debug, Default, Deserialize, Clone)]
pub(crate) struct BaseColor {
    pub(crate) color: Color,
    pub(crate) blending: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct CopyColor {
    pub(crate) copied_index: u64,
    pub(crate) copy_opacity: bool,
    pub(crate) opacity: f32,
    pub(crate) blending: bool,
    pub(crate) hsv: HsvMod,
}

#[derive(Debug, Deserialize, Copy, Clone, Reflect)]
pub(crate) struct HsvMod {
    pub(crate) h: f32,
    pub(crate) s: f32,
    pub(crate) v: f32,
    pub(crate) s_absolute: bool,
    pub(crate) v_absolute: bool,
}

impl HsvMod {
    pub(crate) fn parse(hsv_string: &[u8]) -> Result<HsvMod, anyhow::Error> {
        let hsv_data: [&[u8]; 5] = de::from_slice(hsv_string, b'a')?;
        Ok(HsvMod {
            h: std::str::from_utf8(hsv_data[0])?.parse()?,
            s: std::str::from_utf8(hsv_data[1])?.parse()?,
            v: std::str::from_utf8(hsv_data[2])?.parse()?,
            s_absolute: u8_to_bool(hsv_data[3]),
            v_absolute: u8_to_bool(hsv_data[4]),
        })
    }

    pub(crate) fn apply(&self, color: Color) -> Color {
        let (h, s, v) = rgb_to_hsv([color.r(), color.g(), color.b()]);
        let [r, g, b] = hsv_to_rgb((
            h + self.h,
            if self.s_absolute {
                s + self.s
            } else {
                s * self.s
            },
            if self.v_absolute {
                v + self.v
            } else {
                v * self.v
            },
        ));
        Color::rgba(r, g, b, color.a())
    }
}

impl Default for HsvMod {
    fn default() -> Self {
        HsvMod {
            h: 0.,
            s: 1.,
            v: 1.,
            s_absolute: false,
            v_absolute: false,
        }
    }
}

impl Default for ColorChannel {
    fn default() -> Self {
        ColorChannel::BaseColor(BaseColor::default())
    }
}

impl Default for CopyColor {
    fn default() -> Self {
        CopyColor {
            copied_index: 0,
            copy_opacity: false,
            opacity: 1.,
            blending: false,
            hsv: HsvMod::default(),
        }
    }
}
