use std::path::Path;

use bevy::asset::{
    AssetEvent, AssetLoader, Assets, BoxedFuture, Handle, HandleId, LoadContext, LoadedAsset,
};
use bevy::math::Rect;
use bevy::prelude::{
    Color, Component, EventReader, FromWorld, Image, ResMut, Resource, Vec2, World,
};
use bevy::reflect::{Reflect, TypeUuid};
use bevy::render::{
    renderer::RenderDevice,
    texture::{CompressedImageFormats, ImageType},
};
use bevy::sprite::Anchor;
use bevy::utils::HashMap;
use serde::{Deserialize, Deserializer};

use crate::compressed_image::CompressedImage;
use crate::utils::{fast_scale, linear_to_nonlinear};

#[derive(Debug, TypeUuid)]
#[uuid = "f2c8ed94-b8c8-4d9e-99e9-7ba9b7e8603b"]
pub(crate) struct Cocos2dAtlas {
    pub(crate) texture: Handle<CompressedImage>,
    pub(crate) frames: HashMap<String, Cocos2dFrame>,
}

#[derive(Clone, Debug)]
pub(crate) struct Cocos2dFrame {
    pub(crate) rect: Rect,
    pub(crate) anchor: Vec2,
    pub(crate) rotated: bool,
}

#[derive(Deserialize)]
struct AtlasFile {
    frames: HashMap<String, Frame>,
    metadata: Metadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    format: u8,
    real_texture_file_name: String,
    #[serde(deserialize_with = "to_vec2")]
    size: Vec2,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    aliases: Vec<String>,
    #[serde(deserialize_with = "to_vec2")]
    sprite_offset: Vec2,
    #[serde(deserialize_with = "to_vec2")]
    sprite_size: Vec2,
    #[serde(deserialize_with = "to_vec2")]
    sprite_source_size: Vec2,
    #[serde(deserialize_with = "to_rect")]
    texture_rect: Rect,
    texture_rotated: bool,
}

#[derive(Component, Default, Reflect)]
pub(crate) struct Cocos2dAtlasSprite {
    pub(crate) index: usize,
    pub(crate) color: Color,
    pub(crate) blending: bool,
    pub(crate) flip_x: bool,
    pub(crate) flip_y: bool,
    pub(crate) custom_size: Option<Vec2>,
    pub(crate) anchor: Anchor,
}

#[derive(Resource, Default)]
pub(crate) struct Cocos2dFrames {
    pub(crate) index: HashMap<String, usize>,
    pub(crate) frames: Vec<(Cocos2dFrame, HandleId)>,
}

pub(crate) fn add_frames_to_resource(
    mut frames: ResMut<Cocos2dFrames>,
    mut atlas_events: EventReader<AssetEvent<Cocos2dAtlas>>,
    mut atlases: ResMut<Assets<Cocos2dAtlas>>,
) {
    for atlas_event in atlas_events.iter() {
        match atlas_event {
            AssetEvent::Created { handle } | AssetEvent::Modified { handle } => {
                if let Some(atlas) = atlases.get_mut(handle) {
                    for (texture_name, frame_info) in std::mem::take(&mut atlas.frames) {
                        let frame_index = frames.frames.len();
                        frames.index.insert(texture_name, frame_index);
                        frames.frames.push((frame_info, atlas.texture.id()));
                    }
                }
            }
            AssetEvent::Removed { .. } => {
                // TODO: Remove the frames
            }
        }
    }
}

pub struct Cocos2dAtlasLoader {
    supported_compressed_formats: CompressedImageFormats,
}

impl FromWorld for Cocos2dAtlasLoader {
    fn from_world(world: &mut World) -> Self {
        let supported_compressed_formats = match world.get_resource::<RenderDevice>() {
            Some(render_device) => CompressedImageFormats::from_features(render_device.features()),
            None => CompressedImageFormats::all(),
        };
        Self {
            supported_compressed_formats,
        }
    }
}

impl AssetLoader for Cocos2dAtlasLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let manifest: AtlasFile = plist::from_bytes(bytes).expect("Invalid manifest");
            let mut texture = load_texture(
                load_context,
                &manifest.metadata.real_texture_file_name,
                self.supported_compressed_formats,
            )
            .await?;

            // Premultiply texture
            for pixel in texture.data.chunks_exact_mut(4) {
                // Convert to f32
                let mut f32_alpha = pixel[3] as f32 / u8::MAX as f32;

                // Linear to non-linear
                f32_alpha = linear_to_nonlinear(f32_alpha);

                let non_linear_alpha = (f32_alpha * u8::MAX as f32).round() as u8;

                // Pre-multiply
                pixel[0] = fast_scale(pixel[0], non_linear_alpha);
                pixel[1] = fast_scale(pixel[1], non_linear_alpha);
                pixel[2] = fast_scale(pixel[2], non_linear_alpha);
            }

            let compressed_image = CompressedImage::from_image(texture)?;

            let texture_handle =
                load_context.set_labeled_asset("texture", LoadedAsset::new(compressed_image));
            let mut frames = HashMap::with_capacity(manifest.frames.len());
            for (frame_name, frame) in manifest.frames {
                let frame_rect = if frame.texture_rotated {
                    Rect {
                        min: frame.texture_rect.min,
                        // WTF why does cocos need this i was stuck on this for WEEKS
                        max: Vec2 {
                            x: frame.texture_rect.min.x + frame.sprite_size.y,
                            y: frame.texture_rect.min.y + frame.sprite_size.x,
                        },
                    }
                } else {
                    frame.texture_rect
                };

                // Also WTF is this offset calculation
                let mut anchor = -(frame.sprite_offset / frame.sprite_size);
                if frame.texture_rotated {
                    anchor = Vec2 {
                        x: anchor.y,
                        y: -anchor.x,
                    };
                }

                frames.insert(
                    frame_name,
                    Cocos2dFrame {
                        rect: frame_rect,
                        anchor: anchor * 2.,
                        rotated: frame.texture_rotated,
                    },
                );
            }
            load_context.set_default_asset(LoadedAsset::new(Cocos2dAtlas {
                frames,
                texture: texture_handle,
            }));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["plist"]
    }
}

fn to_vec2<'de, D>(deserializer: D) -> Result<Vec2, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let split_str: Vec<f32> = s
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|str| str.parse().unwrap())
        .collect();
    Ok(Vec2 {
        x: split_str[0],
        y: split_str[1],
    })
}

fn to_rect<'de, D>(deserializer: D) -> Result<Rect, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let dimensions: Vec<f32> = s
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|s| s.trim_matches(|c| c == '{' || c == '}').parse().unwrap())
        .collect();
    Ok(Rect {
        min: Vec2::new(dimensions[0], dimensions[1]),
        max: Vec2::new(dimensions[0] + dimensions[2], dimensions[1] + dimensions[3]),
    })
}

async fn load_texture<'a>(
    load_context: &LoadContext<'a>,
    filename: &str,
    supported_compressed_formats: CompressedImageFormats,
) -> Result<Image, bevy::asset::Error> {
    let parent_dir = load_context.path().parent().unwrap();
    let image_path = parent_dir.join(filename);
    let bytes = load_context.read_asset_bytes(image_path.clone()).await?;
    let extension = Path::new(filename).extension().unwrap().to_str().unwrap();
    let image_type = ImageType::Extension(extension);
    Ok(Image::from_buffer(&bytes, image_type, supported_compressed_formats, true).unwrap())
}
