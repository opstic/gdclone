use bevy::asset::{AssetLoader, BoxedFuture, Handle, LoadContext, LoadedAsset};
use bevy::log::info;
use bevy::math::Rect;
use bevy::prelude::{FromWorld, Image, TextureAtlas, Vec2, World};
use bevy::reflect::TypeUuid;
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{CompressedImageFormats, ImageType};
use bevy::utils::HashMap;
use serde::{Deserialize, Deserializer};
use std::path::Path;

#[derive(Debug, TypeUuid)]
#[uuid = "f2c8ed94-b8c8-4d9e-99e9-7ba9b7e8603b"]
pub struct Cocos2dAtlas {
    pub(crate) index: HashMap<String, (usize, Vec2, bool)>,
    pub(crate) texture_atlas: Handle<TextureAtlas>,
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
            let texture = load_texture(
                load_context,
                &manifest.metadata.real_texture_file_name,
                self.supported_compressed_formats,
            )
            .await?;
            let texture_handle =
                load_context.set_labeled_asset("texture", LoadedAsset::new(texture));
            let mut texture_atlas = TextureAtlas::new_empty(texture_handle, manifest.metadata.size);
            let mut index = HashMap::new();
            for (frame_name, frame) in manifest.frames {
                let texture_index;
                if frame.texture_rotated {
                    texture_index = texture_atlas.add_texture(Rect {
                        min: frame.texture_rect.min,
                        max: Vec2 {
                            x: frame.texture_rect.min.x + frame.sprite_size.y,
                            y: frame.texture_rect.min.y + frame.sprite_size.x
                        },
                    });
                } else {
                    texture_index = texture_atlas.add_texture(Rect {
                        min: frame.texture_rect.min,
                        max: Vec2 {
                            x: frame.texture_rect.min.x + frame.sprite_size.x,
                            y: frame.texture_rect.min.y + frame.sprite_size.y
                        },
                    });
                }

                index.insert(
                    frame_name.clone(),
                    (
                        texture_index,
                        Vec2 {
                            x: frame.sprite_offset.y / frame.sprite_size.y
                                * if frame.texture_rotated { -1. } else { 1. },
                            y: frame.sprite_offset.x / frame.sprite_size.x,
                        },
                        frame.texture_rotated,
                    ),
                );
            }
            let texture_atlas_handle =
                load_context.set_labeled_asset("texture_atlas", LoadedAsset::new(texture_atlas));
            load_context.set_default_asset(LoadedAsset::new(Cocos2dAtlas {
                index,
                texture_atlas: texture_atlas_handle,
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
