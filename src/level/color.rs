use crate::level::object::Object;
use crate::level::{de, Groups};
use crate::loaders::cocos2d_atlas::Cocos2dAtlasSprite;
use crate::utils::{hsv_to_rgb, rgb_to_hsv, u8_to_bool, PassHashMap};
use bevy::prelude::{Color, Query, Res, Resource};
use bevy::reflect::Reflect;
use bevy::render::view::VisibleEntities;
use bevy::utils::HashMap;
use serde::Deserialize;

#[derive(Default, Resource)]
pub(crate) struct ColorChannels(pub(crate) PassHashMap<ColorChannel>);

impl ColorChannels {
    pub(crate) fn get_color(
        &self,
        index: &u64,
        recursed: &mut HashMap<u64, usize>,
    ) -> (Color, bool) {
        match self.0.get(index).unwrap_or(&ColorChannel::default()) {
            ColorChannel::BaseColor(color) => (color.color, color.blending),
            ColorChannel::CopyColor(color) => {
                let mut check = recursed.entry(*index).or_default();
                *check += 1;
                let (original_color, _) = if *check > 3 {
                    (Color::WHITE, false)
                } else {
                    Self::get_color(self, &color.copied_index, recursed)
                };
                let mut transformed_color = color.hsv.apply(original_color);
                if !color.copy_opacity {
                    transformed_color.set_a(color.opacity);
                }
                (transformed_color, color.blending)
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
                temp_color.hsv = Hsv::parse(hsv)?;
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

pub(crate) fn calculate_object_color(
    mut object_query: Query<(&Object, &mut Cocos2dAtlasSprite)>,
    visible_entities_query: Query<&VisibleEntities>,
    groups: Res<Groups>,
    color_channels: Res<ColorChannels>,
) {
    for visible_entities in &visible_entities_query {
        let mut object_iter = object_query.iter_many_mut(&visible_entities.entities);
        'outer: while let Some((object, mut sprite)) = object_iter.fetch_next() {
            let mut opacity = 1.;
            for group_id in &object.groups {
                if let Some(group) = groups.0.get(group_id) {
                    if !group.activated {
                        continue 'outer;
                    }
                    opacity *= group.opacity;
                }
            }
            let (mut color, blending) =
                color_channels.get_color(&object.color_channel, &mut HashMap::new());
            if let Some(hsv) = &object.hsv {
                color = hsv.apply(color);
            }
            color.set_a(color.a() * opacity * object.opacity);
            if object.black {
                color = Color::rgba(0., 0., 0., color.a());
            }
            if blending {
                let transformed_opacity = (0.175656971639325_f64
                    * 7.06033051530761_f64.powf(color.a() as f64)
                    - 0.213355914301931_f64)
                    .clamp(0., 1.) as f32;
                color.set_a(transformed_opacity);
            }
            sprite.color = color;
            sprite.blending = blending;
        }
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
    pub(crate) hsv: Hsv,
}

#[derive(Debug, Deserialize, Copy, Clone, Reflect)]
pub(crate) struct Hsv {
    pub(crate) h: f32,
    pub(crate) s: f32,
    pub(crate) v: f32,
    pub(crate) s_absolute: bool,
    pub(crate) v_absolute: bool,
}

impl Hsv {
    pub(crate) fn parse(hsv_string: &[u8]) -> Result<Hsv, anyhow::Error> {
        let hsv_data: [&[u8]; 5] = de::from_slice(hsv_string, b'a')?;
        Ok(Hsv {
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

impl Default for Hsv {
    fn default() -> Self {
        Hsv {
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
            hsv: Hsv::default(),
        }
    }
}
